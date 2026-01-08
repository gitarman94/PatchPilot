use rocket::{get, post, State, http::Status};
use rocket::serde::json::Json;
use diesel::prelude::*;
use chrono::Utc;

use crate::db::{DbPool, get_conn, log_audit};
use crate::auth::{AuthUser, UserRole};
use crate::models::{Device, NewDevice};
use crate::schema::devices::dsl::*;

/// Get all devices
#[get("/devices")]
pub async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, Status> {
    let pool = pool.inner().clone();
    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = get_conn(&pool);
        let all_devices = devices
            .load::<Device>(&mut conn)
            .map_err(|_| Status::InternalServerError)?;
        Ok(Json(all_devices))
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}

/// Heartbeat endpoint
#[post("/heartbeat")]
pub async fn heartbeat() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "alive" }))
}

/// Get details for a specific device
#[get("/device/<device_id_param>")]
pub async fn get_device_details(
    pool: &State<DbPool>,
    device_id_param: i64,
) -> Result<Json<Device>, Status> {
    let pool = pool.inner().clone();
    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = get_conn(&pool);
        let device = devices
            .filter(id.eq(device_id_param))
            .first::<Device>(&mut conn)
            .map_err(|_| Status::NotFound)?;
        Ok(Json(device))
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}

/// Approve a device
#[post("/approve/<device_id_param>")]
pub async fn approve_device(
    pool: &State<DbPool>,
    device_id_param: i64,
    user: AuthUser,
) -> Result<Status, Status> {
    if !user.has_role(UserRole::Admin) {
        return Err(Status::Unauthorized);
    }
    let username = user.username.clone();
    let pool = pool.inner().clone();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = get_conn(&pool);
        diesel::update(devices.filter(id.eq(device_id_param)))
            .set(approved.eq(true))
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        log_audit(&mut conn, &username, "approve_device", Some(&device_id_param.to_string()), Some("Device approved"))
            .map_err(|_| Status::InternalServerError)?;
        Ok(Status::Ok)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}

/// Register or update a device
#[post("/register_or_update", data = "<info>")]
pub async fn register_or_update_device(
    pool: &State<DbPool>,
    info: Json<NewDevice>,
    user: AuthUser,
) -> Result<Json<serde_json::Value>, Status> {
    if !user.has_role(UserRole::Admin) {
        return Err(Status::Unauthorized);
    }
    let username = user.username.clone();
    let info = info.into_inner();
    let pool = pool.inner().clone();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = get_conn(&pool);

        let existing = devices
            .filter(id.eq(info.device_id))
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
            network_interfaces: info.network_interfaces,
            ip_address: info.ip_address,
        };

        diesel::insert_into(devices)
            .values(&updated)
            .on_conflict(id)
            .do_update()
            .set(&updated)
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        log_audit(&mut conn, &username, "register_or_update_device", Some(&info.device_id.to_string()), Some("Device registered or updated"))
            .map_err(|_| Status::InternalServerError)?;

        Ok(Json(serde_json::json!({
            "device_id": info.device_id,
            "last_checkin": updated.last_checkin.to_string(),
            "status": "ok"
        })))
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}

/// Mount all device-related Rocket routes
pub fn configure_routes(rocket: rocket::Rocket<rocket::Build>) -> rocket::Rocket<rocket::Build> {
    rocket.mount(
        "/",
        routes![
            get_devices,
            heartbeat,
            get_device_details,
            approve_device,
            register_or_update_device
        ],
    )
}
