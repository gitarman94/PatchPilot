use rocket::serde::json::Json;
use rocket::State;
use rocket::http::Status;
use diesel::prelude::*;
use chrono::Utc;

use crate::db::pool::DbPool;
use crate::models::{Device, DeviceInfo, NewDevice};
use crate::schema::devices::dsl::{devices, device_id, approved, last_checkin};

/// Get all devices
#[get("/devices")]
pub async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    let result = devices
        .load::<Device>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
    Ok(Json(result))
}

/// Get details for a specific device
#[get("/device/<device_id_param>")]
pub async fn get_device_details(
    pool: &State<DbPool>,
    device_id_param: &str
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
    device_id_param: &str
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    diesel::update(devices.filter(device_id.eq(device_id_param)))
        .set(approved.eq(true))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
    Ok(Status::Ok)
}

/// Register a new device (client / API)
#[post("/register", data = "<info>")]
pub async fn register_device(
    pool: &State<DbPool>,
    info: Json<DeviceInfo>,
) -> Result<Json<serde_json::Value>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    let new_device = NewDevice::from_device_info(
        &info.device_id,
        &info,
        None, // <--- fix: provide missing third argument
    );

    diesel::insert_into(devices)
        .values(&new_device)
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(serde_json::json!({ "device_id": info.device_id.clone() })))
}

/// Register or update an existing device
#[post("/update", data = "<info>")]
pub async fn register_or_update_device(
    pool: &State<DbPool>,
    info: Json<DeviceInfo>,
) -> Result<Json<serde_json::Value>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    // Load existing device if any
    let existing = devices
        .filter(device_id.eq(&info.device_id))
        .first::<Device>(&mut conn)
        .optional()
        .map_err(|_| Status::InternalServerError)?;

    let updated = NewDevice::from_device_info(
        &info.device_id,
        &info,
        existing.as_ref(),
    );

    diesel::insert_into(devices)
        .values(&updated)
        .on_conflict(device_id)
        .do_update()
        .set(&updated)
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(serde_json::json!({ "device_id": info.device_id.clone() })))
}

/// Heartbeat endpoint
#[post("/heartbeat", data = "<info>")]
pub async fn heartbeat(
    pool: &State<DbPool>,
    info: Json<DeviceInfo>,
) -> Result<Json<serde_json::Value>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    diesel::update(devices.filter(device_id.eq(&info.device_id)))
        .set(last_checkin.eq(Utc::now().naive_utc()))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(serde_json::json!({ "adopted": true })))
}
