#[macro_use] extern crate rocket;
#[macro_use] extern crate diesel;
extern crate serde;
extern crate serde_json;
extern crate chrono;
extern crate env_logger;
extern crate r2d2;
extern crate diesel_r2d2;

use diesel::prelude::*;
use rocket::{State, Response};
use rocket::tokio::sync::Mutex;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::tokio::task::spawn_blocking;
use std::sync::Arc;
use std::time::{Duration, Instant};
use r2d2::PooledConnection;
use diesel::r2d2::ConnectionManager;
use anyhow::{Result, Context};

mod schema;
mod models;

use models::{Device, NewDevice};

#[derive(Deserialize, Serialize)]
struct Heartbeat {
    device_id: String,
    system_info: SystemInfo,
    device_type: Option<String>,
    device_model: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct SystemInfo {
    os_name: String,
    architecture: String,
    cpu: f32,
    ram_total: i64,
    ram_used: i64,
    ram_free: i64,
    disk_total: i64,
    disk_free: i64,
    disk_health: String,
    network_throughput: i64,
    ping_latency: Option<f32>,
}

// Database connection pool type
type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

#[launch]
fn rocket() -> _ {
    env_logger::init();

    // Create a connection pool
    let manager = ConnectionManager::<SqliteConnection>::new("patchpilot.db");
    let pool = r2d2::Pool::builder().build(manager).expect("Failed to create pool.");

    rocket::build()
        .manage(pool)  // Add the connection pool as a managed state
        .mount("/", rocket::fs::FileServer::from("./static"))
        .mount("/api", routes![heartbeat, get_devices, approve_device])
}

fn establish_connection(pool: &DbPool) -> PooledConnection<ConnectionManager<SqliteConnection>> {
    pool.get().expect("Failed to get a DB connection from the pool.")
}

#[post("/devices/heartbeat", format = "json", data = "<heartbeat>")]
async fn heartbeat(
    heartbeat: Json<Heartbeat>,
    pool: &State<DbPool>,
) -> Result<Json<Device>, String> {
    let conn = establish_connection(pool);

    let device_info = heartbeat.into_inner();
    let device_id = device_info.device_id.clone();

    // Using Diesel's upsert functionality (on_conflict) for updating or inserting a device
    let new_device = NewDevice {
        device_name: &device_id,
        hostname: &device_id,
        os_name: &device_info.system_info.os_name,
        architecture: &device_info.system_info.architecture,
        last_checkin: chrono::Utc::now().naive_utc(),
        approved: false,
        device_type: device_info.device_type.unwrap_or_default(),
        device_model: device_info.device_model.unwrap_or_default(),
    };

    // Perform an upsert (insert or update) for the device based on the hostname
    diesel::insert_into(schema::devices::table)
        .values(&new_device)
        .on_conflict(schema::devices::hostname)
        .do_update()
        .set((
            schema::devices::cpu.eq(device_info.system_info.cpu),
            schema::devices::ram_total.eq(device_info.system_info.ram_total),
            schema::devices::ram_used.eq(device_info.system_info.ram_used),
            schema::devices::ram_free.eq(device_info.system_info.ram_free),
            schema::devices::disk_total.eq(device_info.system_info.disk_total),
            schema::devices::disk_free.eq(device_info.system_info.disk_free),
            schema::devices::network_throughput.eq(device_info.system_info.network_throughput),
            schema::devices::ping_latency.eq(device_info.system_info.ping_latency),
        ))
        .execute(&conn)
        .map_err(|e| format!("Failed to upsert device: {}", e))?;

    Ok(Json(Device {
        id: 0,  // Placeholder
        device_name: device_id,
        hostname: device_id,
        os_name: device_info.system_info.os_name,
        architecture: device_info.system_info.architecture,
        last_checkin: chrono::Utc::now().naive_utc(),
        approved: false,  // Placeholder
        cpu: device_info.system_info.cpu,
        ram_total: device_info.system_info.ram_total,
        ram_used: device_info.system_info.ram_used,
        ram_free: device_info.system_info.ram_free,
        disk_total: device_info.system_info.disk_total,
        disk_free: device_info.system_info.disk_free,
        network_throughput: device_info.system_info.network_throughput,
        ping_latency: device_info.system_info.ping_latency,
        device_type: device_info.device_type.unwrap_or_default(),
        device_model: device_info.device_model.unwrap_or_default(),
    }))
}

// Endpoint for approving a device
#[post("/devices/approve", format = "json", data = "<device>")]
async fn approve_device(
    device: Json<Device>,
    pool: &State<DbPool>,
) -> Result<Json<Device>, String> {
    let conn = establish_connection(pool);

    diesel::update(schema::devices::table.filter(schema::devices::hostname.eq(&device.hostname)))
        .set(schema::devices::approved.eq(true))
        .execute(&conn)
        .map_err(|_| "Failed to approve device")?;

    Ok(Json(device.into_inner()))
}

#[get("/devices")]
async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, String> {
    let conn = establish_connection(pool);
    let results = schema::devices::table.load::<Device>(&conn)
        .map_err(|_| "Error loading devices")?;

    Ok(Json(results))
}
