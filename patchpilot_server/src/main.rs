#[macro_use]
extern crate rocket;
#[macro_use]
extern crate diesel;

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use r2d2::Pool;

use rocket::{State};
use rocket::serde::{json::Json, Deserialize, Serialize};
use chrono::Utc;
use anyhow::Result;

mod schema;
mod models;

use models::{Device, NewDevice};

// Type alias for our SQLite connection pool
type DbPool = Pool<ConnectionManager<SqliteConnection>>;

// Establish connection helper
fn establish_connection(pool: &DbPool) -> PooledConnection<ConnectionManager<SqliteConnection>> {
    pool.get().expect("Failed to get a DB connection from the pool.")
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct DeviceInfo {
    pub device_type: Option<String>,
    pub device_model: Option<String>,
    pub system_info: SystemInfo,
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct SystemInfo {
    pub os_name: String,
    pub architecture: String,
    pub cpu: f32,
    pub ram_total: i64,
    pub ram_used: i64,
    pub ram_free: i64,
    pub disk_total: i64,
    pub disk_free: i64,
    pub disk_health: String,
    pub network_throughput: i64,
    pub ping_latency: Option<f32>,
}

#[post("/api/device/<device_id>", format = "json", data = "<device_info>")]
async fn register_or_update_device(
    pool: &State<DbPool>,
    device_id: &str,
    device_info: Json<DeviceInfo>,
) -> Result<Json<Device>, String> {
    use crate::schema::devices::dsl::*;
    let mut conn = establish_connection(pool);

    let new_device = NewDevice {
        device_name: device_id,
        hostname: device_id,
        os_name: &device_info.system_info.os_name,
        architecture: &device_info.system_info.architecture,
        last_checkin: Utc::now().naive_utc(),
        approved: false,
        cpu: device_info.system_info.cpu,
        ram_total: device_info.system_info.ram_total,
        ram_used: device_info.system_info.ram_used,
        ram_free: device_info.system_info.ram_free,
        disk_total: device_info.system_info.disk_total,
        disk_free: device_info.system_info.disk_free,
        disk_health: &device_info.system_info.disk_health,
        network_throughput: device_info.system_info.network_throughput,
        ping_latency: device_info.system_info.ping_latency.unwrap_or(0.0),
        device_type: device_info.device_type.as_deref().unwrap_or(""),
        device_model: device_info.device_model.as_deref().unwrap_or(""),
    };

    // Insert or update existing device
    diesel::insert_into(devices)
        .values(&new_device)
        .on_conflict(device_name)
        .do_update()
        .set((
            cpu.eq(device_info.system_info.cpu),
            ram_total.eq(device_info.system_info.ram_total),
            ram_used.eq(device_info.system_info.ram_used),
            ram_free.eq(device_info.system_info.ram_free),
            disk_total.eq(device_info.system_info.disk_total),
            disk_free.eq(device_info.system_info.disk_free),
            network_throughput.eq(device_info.system_info.network_throughput),
            ping_latency.eq(device_info.system_info.ping_latency.unwrap_or(0.0)),
            last_checkin.eq(Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map_err(|e| e.to_string())?;

    // Return the updated record
    let result = devices
        .filter(device_name.eq(device_id))
        .first::<Device>(&mut conn)
        .map_err(|e| e.to_string())?;

    Ok(Json(result))
}

#[get("/api/devices")]
async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, String> {
    use crate::schema::devices::dsl::*;
    let mut conn = establish_connection(pool);

    let results = devices
        .load::<Device>(&mut conn)
        .map_err(|e| e.to_string())?;

    Ok(Json(results))
}

#[launch]
fn rocket() -> _ {
    use std::env;

    env::set_var("RUST_LOG", "info");
    env_logger::init();

    // Set up database pool
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = Pool::builder()
        .build(manager)
        .expect("Failed to create DB pool");

    rocket::build()
        .manage(pool)
        .mount("/", routes![register_or_update_device, get_devices])
}
