#[macro_use] extern crate rocket;
#[macro_use] extern crate diesel;
extern crate serde;
extern crate serde_json;
extern crate chrono;
extern crate env_logger;

use diesel::prelude::*;
use rocket::{State, Response};
use rocket::tokio::sync::Mutex;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::tokio::task::spawn_blocking;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod schema;
mod models;

use models::{Device, NewDevice};
use rocket::fs::FileServer;

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

#[launch]
fn rocket() -> _ {
    env_logger::init();
    rocket::build()
        .mount("/", FileServer::from("./static"))
        .mount("/api", routes![heartbeat, get_devices, approve_device])
        .manage(Arc::new(Mutex::new(establish_connection())))
}

fn establish_connection() -> SqliteConnection {
    let database_url = "patchpilot.db";
    SqliteConnection::establish(&database_url).expect("Error connecting to the database")
}

#[post("/devices/heartbeat", format = "json", data = "<heartbeat>")]
async fn heartbeat(heartbeat: Json<Heartbeat>, db: &State<Arc<Mutex<SqliteConnection>>>) -> Result<Json<Device>, String> {
    let conn = db.lock().await;

    let device_info = heartbeat.into_inner();
    let device_id = device_info.device_id.clone();

    // Check if device exists
    let device = diesel::select(diesel::dsl::exists(
        schema::devices::table.filter(schema::devices::hostname.eq(&device_id))
    ))
    .get_result(&*conn)
    .map_err(|_| "Failed to query the device")?;

    if device {
        // Update the device if it exists
        diesel::update(schema::devices::table.filter(schema::devices::hostname.eq(&device_id)))
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
            .execute(&*conn)
            .map_err(|_| "Failed to update device")?;
    } else {
        // Register a new device if it doesn't exist
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

        diesel::insert_into(schema::devices::table)
            .values(&new_device)
            .execute(&*conn)
            .map_err(|_| "Failed to insert new device")?;
    }

    Ok(Json(Device {
        id: 0,  // placeholder
        device_name: device_id,
        hostname: device_id,
        os_name: device_info.system_info.os_name,
        architecture: device_info.system_info.architecture,
        last_checkin: chrono::Utc::now().naive_utc(),
        approved: false,  // placeholder
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
async fn approve_device(device: Json<Device>, db: &State<Arc<Mutex<SqliteConnection>>>) -> Result<Json<Device>, String> {
    let conn = db.lock().await;

    diesel::update(schema::devices::table.filter(schema::devices::hostname.eq(&device.hostname)))
        .set(schema::devices::approved.eq(true))
        .execute(&*conn)
        .map_err(|_| "Failed to approve device")?;

    Ok(Json(device.into_inner()))
}

#[get("/devices")]
async fn get_devices(db: &State<Arc<Mutex<SqliteConnection>>>) -> Json<Vec<Device>> {
    let conn = db.lock().await;
    let results = schema::devices::table.load::<Device>(&*conn).expect("Error loading devices");

    Json(results)
}

