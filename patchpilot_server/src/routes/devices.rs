use diesel::prelude::*;
use rocket::{get, post, State};
use rocket::serde::json::Json;
use rocket::http::Status;

use crate::DbPool;
use crate::models::{Device, DeviceInfo};
use crate::schema::devices::dsl::*;

#[get("/devices")]
pub async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    let rows = devices
        .order(hostname.asc())
        .load::<Device>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(rows))
}

#[get("/device/<device_id>")]
pub async fn get_device_details(
    pool: &State<DbPool>,
    device_id: &str,
) -> Result<Json<Device>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    let dev = devices
        .filter(uuid.eq(device_id))
        .first::<Device>(&mut conn)
        .map_err(|_| Status::NotFound)?;

    Ok(Json(dev))
}

#[post("/approve/<device_id>")]
pub async fn approve_device(
    pool: &State<DbPool>,
    device_id: &str,
) -> Result<Json<serde_json::Value>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    diesel::update(devices.filter(uuid.eq(device_id)))
        .set(approved.eq(true))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(serde_json::json!({
        "status": "approved",
        "device_uuid": device_id
    })))
}
