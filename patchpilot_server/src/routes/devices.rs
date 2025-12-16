use diesel::prelude::*;
use rocket::{get, post, serde::json::Json, State};
use rocket::http::Status;

use crate::db::pool::DbPool;
use crate::models::{Device, NewDevice};
use crate::schema::devices::dsl::*;

#[get("/api/devices")]
pub async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    let rows = devices
        .order(name.asc())
        .load::<Device>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(rows))
}

#[get("/api/device/<uuid>")]
pub async fn get_device_details(
    pool: &State<DbPool>,
    uuid: &str,
) -> Result<Json<Device>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    let device = devices
        .filter(device_uuid.eq(uuid))
        .first::<Device>(&mut conn)
        .map_err(|_| Status::NotFound)?;

    Ok(Json(device))
}

#[post("/api/device/approve/<uuid>")]
pub async fn approve_device(
    pool: &State<DbPool>,
    uuid: &str,
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    diesel::update(devices.filter(device_uuid.eq(uuid)))
        .set(approved.eq(true))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Status::Ok)
}

#[post("/api/device/register", data = "<new_device>")]
pub async fn register_device(
    pool: &State<DbPool>,
    new_device: Json<NewDevice>,
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    diesel::insert_into(devices)
        .values(&*new_device)
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Status::Created)
}

#[post("/api/device/heartbeat/<uuid>")]
pub async fn heartbeat(
    pool: &State<DbPool>,
    uuid: &str,
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    diesel::update(devices.filter(device_uuid.eq(uuid)))
        .set(last_seen.eq(chrono::Utc::now().naive_utc()))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Status::Ok)
}
