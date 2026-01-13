use rocket::{get, post, delete, routes, State};
use rocket::form::Form;
use rocket::http::Status;
use rocket::response::Redirect;
use rocket::serde::json::Json;
use rocket_dyn_templates::Template;

use diesel::prelude::*;
use chrono::{Utc, NaiveDateTime};
use std::sync::Arc;

use crate::db::{DbPool, db_log_audit};
use crate::models::{Device, NewDevice, AppState};
use crate::schema::devices::dsl::*;
use crate::auth::{AuthUser, RoleName};


/// GET /api/devices - return all devices
#[get("/devices")]
pub async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, Status> {
    let pool = pool.inner().clone();
    let devices_res = rocket::tokio::task::spawn_blocking(move || -> Result<Vec<Device>, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let all_devices = devices
            .load::<Device>(&mut conn)
            .map_err(|_| Status::InternalServerError)?;
        Ok(all_devices)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;
    Ok(Json(devices_res))
}

/// Heartbeat endpoint
#[post("/heartbeat")]
pub async fn heartbeat() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "alive" }))
}

/// GET /api/device/<device_id>
#[get("/device/<device_id_param>")]
pub async fn get_device_details(pool: &State<DbPool>, device_id_param: i64) -> Result<Json<Device>, Status> {
    let pool = pool.inner().clone();
    let device = rocket::tokio::task::spawn_blocking(move || -> Result<Device, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let found = devices
            .filter(device_id.eq(device_id_param))
            .first::<Device>(&mut conn)
            .map_err(|_| Status::NotFound)?;
        Ok(found)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;
    Ok(Json(device))
}

/// POST /api/approve/<device_id> - approve a device (admin only)
#[post("/approve/<device_id_param>")]
pub async fn approve_device(pool: &State<DbPool>, device_id_param: i64, user: AuthUser) -> Result<Status, Status> {
    if !user.has_role(RoleName::Admin) {
        return Err(Status::Unauthorized);
    }
    let username = user.username.clone();
    let pool = pool.inner().clone();

    // handle res properly
    rocket::tokio::task::spawn_blocking(move || -> Result<(), Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        diesel::update(devices.filter(device_id.eq(device_id_param)))
            .set(approved.eq(true))
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;
        let _ = log_audit(
            &mut conn,
            &username,
            "approve_device",
            Some(&device_id_param.to_string()),
            Some("Device approved"),
        );
        Ok(())
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Status::Ok)
}

/// Incoming device JSON payload
#[derive(Debug, serde::Deserialize)]
pub struct DeviceIncoming {
    pub device_id: i64,
    pub device_name: String,
    pub hostname: String,
    pub os_name: String,
    pub architecture: String,
    pub cpu_usage: f32,
    pub cpu_count: i32,
    pub cpu_brand: String,
    pub ram_total: i64,
    pub ram_used: i64,
    pub disk_total: i64,
    pub disk_free: i64,
    pub disk_health: String,
    pub network_throughput: i64,
    pub device_type: String,
    pub device_model: String,
    pub uptime: Option<i64>,
    pub updates_available: bool,
    pub network_interfaces: Option<serde_json::Value>,
    pub ip_address: Option<String>,
}

/// POST /api/register_or_update - register or update a device
#[post("/register_or_update", data = "<info>")]
pub async fn register_or_update_device(
    pool: &State<DbPool>,
    app_state: &State<Arc<AppState>>,
    info: Json<DeviceIncoming>,
    user: AuthUser,
) -> Result<Json<serde_json::Value>, Status> {
    // require admin
    if !user.has_role(RoleName::Admin) {
        return Err(Status::Unauthorized);
    }

    let username = user.username.clone();
    let info = info.into_inner();
    let pool_for_db = pool.inner().clone();
    let app_state = app_state.inner().clone();

    let result = rocket::tokio::task::spawn_blocking(move || -> Result<serde_json::Value, Status> {
        let mut conn = pool_for_db.get().map_err(|_| Status::InternalServerError)?;
        // Try to find existing by device_id
        let existing = devices
            .filter(device_id.eq(info.device_id))
            .first::<Device>(&mut conn)
            .optional()
            .map_err(|_| Status::InternalServerError)?;

        let updated = NewDevice {
            device_id: info.device_id,
            device_name: info.device_name,
            hostname: info.hostname,
            os_name: info.os_name,
            architecture: info.architecture,
            last_checkin: Utc::now().naive_utc(),
            approved: existing.as_ref().map_or(false, |d| d.approved),
            cpu_usage: info.cpu_usage,
            cpu_count: info.cpu_count,
            cpu_brand: info.cpu_brand,
            ram_total: info.ram_total,
            ram_used: info.ram_used,
            disk_total: info.disk_total,
            disk_free: info.disk_free,
            disk_health: info.disk_health,
            network_throughput: info.network_throughput,
            device_type: info.device_type,
            device_model: info.device_model,
            uptime: info.uptime,
            updates_available: info.updates_available,
            network_interfaces: info.network_interfaces.map(|v| v.to_string()),
            ip_address: info.ip_address,
        };

        // Upsert by device_id
        diesel::insert_into(devices)
            .values(&updated)
            .on_conflict(device_id)
            .do_update()
            .set(&updated)
            .execute(&mut conn)
            .map_err(|e| {
                log::error!("DB insert/update failed: {:?}", e);
                Status::InternalServerError
            })?;

        let _ = log_audit(
            &mut conn,
            &username,
            "register_or_update_device",
            Some(&updated.device_id.to_string()),
            Some("Device registered or updated"),
        );

        // Mark device as recently seen for pending cleanup tracking
        app_state.update_pending_device(&updated.device_id.to_string());

        Ok(serde_json::json!({
            "device_id": updated.device_id,
            "last_checkin": updated.last_checkin.to_string(),
            "status": "ok"
        }))
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(result))
}

use rocket::Route;

pub fn routes() -> Vec<Route> {
    routes![
        get_devices,
        heartbeat,
        get_device_details,
        approve_device,
        register_or_update_device
    ]
    .into_iter()
    .collect()
}
