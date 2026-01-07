use rocket::serde::json::Json;
use rocket::{State, http::Status, get, post};
use diesel::prelude::*;
use chrono::Utc;

use crate::db::{DbPool, log_audit};
use crate::auth::{AuthUser, UserRole};
use crate::models::{Device, DeviceInfo, NewDevice, ServerSettings as ModelServerSettings};
use crate::schema::devices::dsl::*;

/// Helper: load server settings from DB
pub async fn get_server_settings(pool: &State<DbPool>) -> ModelServerSettings {
    let pool_clone = pool.inner().clone();
    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool_clone.get().expect("Failed to get DB connection");
        let settings = crate::settings::ServerSettings::load(&mut conn);
        ModelServerSettings {
            id: 0,
            auto_approve_devices: settings.auto_approve_devices,
            auto_refresh_enabled: settings.auto_refresh_enabled,
            auto_refresh_seconds: settings.auto_refresh_seconds,
            default_action_ttl_seconds: settings.default_action_ttl_seconds,
            action_polling_enabled: settings.action_polling_enabled,
            ping_target_ip: settings.ping_target_ip,
            force_https: settings.force_https,
            allow_http: settings.allow_http,
        }
    })
    .await
    .expect("Failed to load server settings")
}

/// Get all devices
#[get("/devices")]
pub async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, Status> {
    let pool_clone = pool.inner().clone();
    let devices_list = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool_clone.get().map_err(|_| Status::InternalServerError)?;
        devices.load::<Device>(&mut conn).map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(devices_list))
}

/// Get details for a specific device
#[get("/device/<device_id_param>")]
pub async fn get_device_details(
    pool: &State<DbPool>,
    device_id_param: &str,
) -> Result<Json<Device>, Status> {
    let device_id_str = device_id_param.to_string();
    let pool_clone = pool.inner().clone();
    let device_opt = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool_clone.get().map_err(|_| Status::InternalServerError)?;
        devices
            .filter(device_id.eq(&device_id_str))
            .first::<Device>(&mut conn)
            .map_err(|_| Status::NotFound)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(device_opt))
}

/// Approve a device
#[post("/approve/<device_id_param>")]
pub async fn approve_device(
    pool: &State<DbPool>,
    device_id_param: &str,
    user: AuthUser,
) -> Result<Status, Status> {
    if !user.has_role(UserRole::Admin) {
        return Err(Status::Unauthorized);
    }

    let username = user.username.clone();
    let device_id_str = device_id_param.to_string();
    let pool_clone = pool.inner().clone();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool_clone.get().map_err(|_| Status::InternalServerError)?;
        diesel::update(devices.filter(device_id.eq(&device_id_str)))
            .set(crate::schema::devices::approved.eq(true))
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        log_audit(
            &mut conn,
            &username,
            "approve_device",
            Some(&device_id_str),
            Some("Device approved"),
        )
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
    info: Json<DeviceInfo>,
    user: AuthUser,
) -> Result<Json<serde_json::Value>, Status> {
    if !user.has_role(UserRole::Admin) {
        return Err(Status::Unauthorized);
    }

    let username = user.username.clone();
    let info = info.into_inner();
    let pool_inner = pool.inner().clone();

    let settings = get_server_settings(pool).await;

    let result = rocket::tokio::task::spawn_blocking(move || -> Result<serde_json::Value, Status> {
        let mut conn = pool_inner.get().map_err(|_| Status::InternalServerError)?;
        let existing = devices
            .filter(device_id.eq(&info.device_id))
            .first::<Device>(&mut conn)
            .optional()
            .map_err(|_| Status::InternalServerError)?;

        let mut updated = NewDevice {
            device_id: info.device_id.clone(),
            device_name: info.system_info.os_name.clone(),
            hostname: info.system_info.os_name.clone(),
            os_name: info.system_info.os_name.clone(),
            architecture: info.system_info.architecture.clone(),
            last_checkin: Utc::now().naive_utc(),
            approved: existing.as_ref().map_or(false, |d| d.approved),
            cpu_usage: info.system_info.cpu_usage,
            cpu_count: info.system_info.cpu_count,
            cpu_brand: info.system_info.cpu_brand.clone(),
            ram_total: info.system_info.ram_total,
            ram_used: info.system_info.ram_used,
            disk_total: info.system_info.disk_total,
            disk_free: info.system_info.disk_free,
            disk_health: info.system_info.disk_health.clone(),
            network_throughput: info.system_info.network_throughput,
            device_type: info.device_type.unwrap_or_default(),
            device_model: info.device_model.unwrap_or_default(),
            uptime: None,
            updates_available: false,
            network_interfaces: info.system_info.network_interfaces.clone(),
            ip_address: info.system_info.ip_address.clone(),
        };

        if settings.auto_approve_devices {
            updated.approved = true;
        }

        diesel::insert_into(devices)
            .values(&updated)
            .on_conflict(device_id)
            .do_update()
            .set(&updated)
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        log_audit(
            &mut conn,
            &username,
            "register_or_update_device",
            Some(&info.device_id),
            Some("Device registered or updated"),
        )
        .map_err(|_| Status::InternalServerError)?;

        Ok(serde_json::json!({
            "device_id": info.device_id,
            "last_checkin": updated.last_checkin.to_string(),
            "status": "ok"
        }))
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(result))
}
