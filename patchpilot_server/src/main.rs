use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use r2d2::Pool;
use rocket::{get, post, routes, launch, State};
use rocket::serde::json::Json;
use rocket::fs::{FileServer, NamedFile};
use flexi_logger::{Logger, FileSpec, Age, Cleanup, Criterion, Naming};
use log::info;
use serde_json::json;
use chrono::Utc;
use local_ip_address::local_ip;
use std::sync::Mutex;

use sysinfo::System;

mod schema;
mod models;

use models::{Device, NewDevice, DeviceInfo};
use diesel::sqlite::SqliteConnection;

type DbPool = Pool<ConnectionManager<SqliteConnection>>;

pub struct AppState {
    pub system: Mutex<System>,
}

fn init_logger() {
    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(FileSpec::default().directory("logs"))
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Numbers,
            Cleanup::KeepLogFiles(7),
        )
        .start()
        .unwrap();
}

#[derive(Debug)]
pub enum ApiError {
    DbError(diesel::result::Error),
    ValidationError(String),
}

impl From<diesel::result::Error> for ApiError {
    fn from(e: diesel::result::Error) -> Self {
        ApiError::DbError(e)
    }
}

impl ApiError {
    fn message(&self) -> String {
        match self {
            ApiError::DbError(e) => format!("Database error: {}", e),
            ApiError::ValidationError(msg) => msg.clone(),
        }
    }
}

fn establish_connection(pool: &DbPool) -> Result<PooledConnection<ConnectionManager<SqliteConnection>>, ApiError> {
    pool.get().map_err(|e| ApiError::ValidationError(format!("Failed to get DB connection: {}", e)))
}

fn validate_device_info(info: &DeviceInfo) -> Result<(), ApiError> {
    if info.system_info.cpu < 0.0 {
        return Err(ApiError::ValidationError("CPU usage cannot be negative".into()));
    }
    if info.system_info.ram_total <= 0 {
        return Err(ApiError::ValidationError("RAM total must be positive".into()));
    }
    Ok(())
}

fn insert_or_update_device(
    conn: &mut SqliteConnection,
    device_id: &str,
    info: &DeviceInfo
) -> Result<Device, ApiError> {
    use crate::schema::devices::dsl::*;

    let new_device = NewDevice::from_device_info(device_id, info);

    diesel::insert_into(devices)
        .values(&new_device)
        .on_conflict(device_name)
        .do_update()
        .set(&new_device)
        .execute(conn)?;

    let updated_device = devices
        .filter(device_name.eq(device_id))
        .first::<Device>(conn)?;

    Ok(updated_device.enrich_for_dashboard())
}

#[post("/devices/<device_id>", format = "json", data = "<device_info>")]
async fn register_or_update_device(
    pool: &State<DbPool>,
    device_id: &str,
    device_info: Json<DeviceInfo>,
) -> Result<Json<Device>, String> {
    validate_device_info(&device_info).map_err(|e| e.message())?;

    let pool = pool.inner().clone();
    let device_info = device_info.into_inner();
    let device_id = device_id.to_string();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = establish_connection(&pool).map_err(|e| e.message())?;
        insert_or_update_device(&mut conn, &device_id, &device_info)
            .map(Json)
            .map_err(|e| e.message())
    })
    .await
    .unwrap_or_else(|e| Err(format!("Task join error: {}", e)))
}

#[post("/devices/heartbeat", format = "json", data = "<payload>")]
async fn heartbeat(
    pool: &State<DbPool>,
    payload: Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    use crate::schema::devices::dsl::*;

    let pool = pool.inner().clone();
    let payload = payload.into_inner();

    rocket::tokio::task::spawn_blocking(move || {
        if let Ok(mut conn) = pool.get() {
            let device_id = payload.get("device_id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let device_type_val = payload.get("device_type").and_then(|v| v.as_str()).unwrap_or("unknown");
            let device_model_val = payload.get("device_model").and_then(|v| v.as_str()).unwrap_or("unknown");
            let network_interfaces_val = payload.get("network_interfaces").and_then(|v| v.as_str()).unwrap_or("");
            let ip_address_val = payload.get("ip_address").and_then(|v| v.as_str()).unwrap_or("");

            let _ = diesel::insert_into(devices)
                .values((
                    device_name.eq(device_id),
                    hostname.eq(device_id),              // REQUIRED — NOT NULL
                    os_name.eq("unknown"),               // REQUIRED — NOT NULL
                    architecture.eq("unknown"),          // REQUIRED — NOT NULL
                    device_type.eq(device_type_val),
                    device_model.eq(device_model_val),
                    network_interfaces.eq(network_interfaces_val),
                    ip_address.eq(ip_address_val),
                    approved.eq(true),
                    last_checkin.eq(Utc::now().naive_utc()),
                    cpu.eq(0.0),
                    ram_total.eq(0),
                    ram_used.eq(0),
                    ram_free.eq(0),
                    disk_total.eq(0),
                    disk_free.eq(0),
                    disk_health.eq("unknown"),
                    network_throughput.eq(0),
                    ping_latency.eq(None::<f32>),
                    uptime.eq(Some("0h 0m")),
                    updates_available.eq(false)
                ))
                .on_conflict(device_name)
                .do_update()
                .set((
                    last_checkin.eq(Utc::now().naive_utc()),
                    network_interfaces.eq(network_interfaces_val),
                    ip_address.eq(ip_address_val),
                ))
                .execute(&mut conn);
        }
    })
    .await.ok();

    Json(json!({"adopted": true}))
}

#[get("/devices")]
async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, String> {
    let pool = pool.inner().clone();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = establish_connection(&pool).map_err(|e| e.message())?;
        let results = crate::schema::devices::dsl::devices
            .load::<Device>(&mut conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|d| d.enrich_for_dashboard())
            .collect::<Vec<_>>();
        Ok(Json(results))
    })
    .await
    .unwrap_or_else(|e| Err(format!("Task join error: {}", e)))
}

#[get("/status")]
fn status(state: &State<AppState>) -> Json<serde_json::Value> {
    // Lock the system mutex
    let mut sys = state.system.lock().unwrap();

    // Refresh system information
    sys.refresh_all();

    // Memory and swap info
    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let total_swap = sys.total_swap();
    let used_swap = sys.used_swap();

    let memory_usage_percent = if total_memory > 0 {
        (used_memory as f32 / total_memory as f32) * 100.0
    } else {
        0.0
    };

    let swap_usage_percent = if total_swap > 0 {
        (used_swap as f32 / total_swap as f32) * 100.0
    } else {
        0.0
    };

    // Build JSON response
    Json(json!({
        "server_time": Utc::now().to_rfc3339(),
        "status": "ok",
        "uptime_seconds": sysinfo::System::uptime(),
        "cpu_count": sys.cpus().len(),
        "cpu_usage_per_core_percent": sys.cpus().iter().map(|c| c.cpu_usage()).collect::<Vec<f32>>(),
        "total_memory_bytes": total_memory,
        "used_memory_bytes": used_memory,
        "memory_usage_percent": memory_usage_percent,
        "total_swap_bytes": total_swap,
        "used_swap_bytes": used_swap,
        "swap_usage_percent": swap_usage_percent,
    }))
}

#[get("/")]
async fn dashboard() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/templates/dashboard.html").await.ok()
}

#[get("/favicon.ico")]
async fn favicon() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/static/favicon.ico").await.ok()
}

fn initialize_db(conn: &mut SqliteConnection) -> Result<(), diesel::result::Error> {
    diesel::sql_query(r#"
        CREATE TABLE IF NOT EXISTS devices (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device_name TEXT NOT NULL UNIQUE,
            hostname TEXT,
            os_name TEXT,
            architecture TEXT,
            last_checkin TIMESTAMP NOT NULL,
            approved BOOLEAN NOT NULL,
            cpu FLOAT NOT NULL DEFAULT 0.0,
            ram_total BIGINT NOT NULL DEFAULT 0,
            ram_used BIGINT NOT NULL DEFAULT 0,
            ram_free BIGINT NOT NULL DEFAULT 0,
            disk_total BIGINT NOT NULL DEFAULT 0,
            disk_free BIGINT NOT NULL DEFAULT 0,
            disk_health TEXT,
            network_throughput BIGINT NOT NULL DEFAULT 0,
            ping_latency FLOAT,
            device_type TEXT NOT NULL,
            device_model TEXT NOT NULL,
            uptime TEXT,
            updates_available BOOLEAN NOT NULL DEFAULT 0,
            network_interfaces TEXT,
            ip_address TEXT
        )
    "#).execute(conn)?;
    Ok(())
}

fn get_server_ip() -> String {
    local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "127.0.0.1".into())
}

#[launch]
fn rocket() -> _ {
    init_logger();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = Pool::builder().build(manager).expect("Failed to create DB pool");

    {
        let mut conn = pool.get().expect("DB connect failed");
        initialize_db(&mut conn).expect("DB init failed");
        info!("Database ready");
    }

    let ip = get_server_ip();
    let port = 8080;
    info!("Server running at http://{}:{}/", ip, port);

    rocket::build()
        .manage(pool)
        .manage(AppState {
            system: Mutex::new(System::new_all()),
        })
        .mount(
            "/api",
            routes![
                register_or_update_device,
                get_devices,
                status,
                heartbeat
            ],
        )
        .mount("/", routes![dashboard, favicon])
        .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
}
