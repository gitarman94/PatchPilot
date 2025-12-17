use rocket::serde::json::Json;
use rocket::State;
use rocket::http::Status;
use diesel::prelude::*;
use chrono::Utc;

use crate::db::pool::DbPool;
use crate::models::{Device, DeviceInfo};
use crate::schema::devices::dsl::*;

/// Get all devices
#[get("/devices")]
pub async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    let result = devices.load::<Device>(&mut conn).map_err(|_| Status::InternalServerError)?;
    Ok(Json(result))
}

/// Get details for a specific device
#[get("/device/<device_uuid>")]
pub async fn get_device_details(pool: &State<DbPool>, device_uuid: &str) -> Result<Json<Device>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    let device = devices.filter(uuid.eq(device_uuid))
        .first::<Device>(&mut conn)
        .map_err(|_| Status::NotFound)?;
    Ok(Json(device))
}

/// Approve a device
#[post("/approve/<device_uuid>")]
pub async fn approve_device(pool: &State<DbPool>, device_uuid: &str) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    diesel::update(devices.filter(uuid.eq(device_uuid)))
        .set(approved.eq(true))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
    Ok(Status::Ok)
}

/// Register a new device (from client)
#[post("/register", data = "<info>")]
pub async fn register_device(
    pool: &State<DbPool>,
    info: Json<DeviceInfo>,
) -> Result<Json<serde_json::Value>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    let new_device = diesel::insert_into(devices)
        .values(&*info)
        .get_result::<Device>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
    
    Ok(Json(serde_json::json!({ "device_id": new_device.uuid })))
}

/// Register or update an existing device
#[post("/update", data = "<info>")]
pub async fn register_or_update_device(
    pool: &State<DbPool>,
    info: Json<DeviceInfo>,
) -> Result<Json<serde_json::Value>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    diesel::insert_into(devices)
        .values(&*info)
        .on_conflict(uuid)
        .do_update()
        .set((
            hostname.eq(&info.hostname),
            approved.eq(info.approved),
        ))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
    
    let dev = devices.filter(uuid.eq(&info.uuid))
        .first::<Device>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
    
    Ok(Json(serde_json::json!({ "device_id": dev.uuid })))
}

/// Heartbeat endpoint
#[post("/heartbeat", data = "<info>")]
pub async fn heartbeat(
    pool: &State<DbPool>,
    info: Json<DeviceInfo>,
) -> Result<Json<serde_json::Value>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    diesel::update(devices.filter(uuid.eq(&info.uuid)))
        .set(last_seen.eq(Utc::now().naive_utc()))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
    
    Ok(Json(serde_json::json!({ "adopted": true })))
}
