use rocket::serde::json::Json;
use rocket::{State, http::Status, get, post};
use diesel::prelude::*;
use chrono::Utc;

use crate::db::{DbPool, log_audit, load_settings};
use crate::auth::{AuthUser, UserRole};
use crate::models::{Device, DeviceInfo, NewDevice, ServerSettings as ModelServerSettings};
use crate::schema::devices::dsl::*;

/// Get all devices
#[get("/devices")]
pub async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    let result = devices.load::<Device>(&mut conn).map_err(|_| Status::InternalServerError)?;
    Ok(Json(result))
}

/// Get details for a specific device
#[get("/device/<device_id_param>")]
pub async fn get_device_details(
    pool: &State<DbPool>,
    device_id_param: &str,
) -> Result<Json<Device>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    let device = devices
        .filter(device_id.eq(device_id_param))
        .first::<Device>(&mut conn)
        .map_err(|_| Status::NotFound)?;
    Ok(Json(device))
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

    let settings = crate::routes::devices::get_server_settings(pool)
        .await
        .unwrap_or_default();

    let result = rocket::tokio::task::spawn_blocking(move || -> Result<serde_json::Value, Status> {
        let mut conn = pool_inner.get().map_err(|_| Status::InternalServerError)?;
        let existing = devices
            .filter(device_id.eq(&info.device_id))
            .first::<Device>(&mut conn)
            .optional()
            .map_err(|_| Status::InternalServerError)?;

        let mut updated = NewDevice::from_device_info(&info.device_id, &info, existing.as_ref());
        updated.last_checkin = Utc::now().naive_utc();

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
