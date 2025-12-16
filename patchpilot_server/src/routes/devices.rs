use diesel::prelude::*;
use rocket::{get, post, State};
use rocket::http::Status;

use crate::db::pool::DbPool;
use crate::models::Device;
use crate::schema::devices::dsl::*;

#[get("/api/devices")]
pub async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    let rows = devices
        .order(hostname.asc())
        .load::<Device>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(rows))
}

#[get("/api/device/<device_id>")]
pub async fn get_device_details(
    pool: &State<DbPool>,
    device_id: &str,
) -> Result<Json<Device>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    let device = devices
        .filter(uuid.eq(device_id))
        .first::<Device>(&mut conn)
        .map_err(|_| Status::NotFound)?;

    Ok(Json(device))
}

#[post("/api/device/approve/<device_id>")]
pub async fn approve_device(
    pool: &State<DbPool>,
    device_id: &str,
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    diesel::update(devices.filter(uuid.eq(device_id)))
        .set(approved.eq(true))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Status::Ok)
}
