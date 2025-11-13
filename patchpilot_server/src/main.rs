use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use r2d2::Pool;
use rocket::{get, post, routes, launch, State};
use rocket::serde::json::Json;
use rocket::fs::{FileServer, NamedFile};
use flexi_logger::{Logger, FileSpec, Age, Cleanup, Criterion, Naming};
use log::{info, error};
use serde_json::json;
use chrono::Utc;
use local_ip_address::local_ip;
use sysinfo::{System, SystemExt, CpuExt};

mod schema;
mod models;

use models::{Device, NewDevice, DeviceInfo};
use diesel::sqlite::SqliteConnection;

// Type alias for SQLite connection pool
type DbPool = Pool<ConnectionManager<SqliteConnection>>;

// Shared system stats struct
struct AppState {
    db_pool: DbPool,
    system: std::sync::Mutex<System>,
}

// --- Logging initialization ---
fn init_logger() {
    let logger = Logger::try_with_str("info")
        .unwrap()
        .log_to_file(FileSpec::default().directory("logs"))
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Numbers,
            Cleanup::KeepLogFiles(7),
        );

    logger.start().unwrap();
}

// --- DB helpers ---
fn establish_connection(pool: &DbPool) -> Result<PooledConnection<ConnectionManager<SqliteConnection>>, String> {
    pool.get().map_err(|e| format!("Failed to get DB connection: {}", e))
}

fn validate_device_info(info: &DeviceInfo) -> Result<(), String> {
    if info.system_info.cpu < 0.0 {
        return Err("CPU usage cannot be negative".into());
    }
    if info.system_info.ram_total <= 0 {
        return Err("RAM total must be positive".into());
    }
    Ok(())
}

fn insert_or_update_device(conn: &mut SqliteConnection, device_id: &str, info: &DeviceInfo) -> Result<Device, String> {
    use crate::schema::devices::dsl::*;

    let new_device = NewDevice::from_device_info(device_id, info);

    diesel::insert_into(devices)
        .values(&new_device)
        .on_conflict(device_name)
        .do_update()
        .set(&new_device)
        .execute(conn)
        .map_err(|e| e.to_string())?;

    let updated_device = devices
        .filter(device_name.eq(device_id))
        .first::<Device>(conn)
        .map_err(|e| e.to_string())?;

    Ok(updated_device.enrich_for_dashboard())
}

// --- REST API endpoints ---
#[post("/devices/<device_id>", format = "json", data = "<device_info>")]
async fn register_or_update_device(
    state: &State<AppState>,
    device_id: &str,
    device_info: Json<DeviceInfo>,
) -> Result<Json<Device>, String> {
    validate_device_info(&device_info)?;
    let mut conn = establish_connection(&state.db_pool)?;
    let device = insert_or_update_device(&mut conn, device_id, &device_info)?;
    info!("Device {} registered/updated successfully", device_id);
    Ok(Json(device))
}

#[post("/devices/heartbeat", format = "json", data = "<payload>")]
async fn heartbeat(state: &State<AppState>, payload: Json<serde_json::Value>) -> Json<serde_json::Value> {
    use crate::schema::devices::dsl::*;

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");

    let device_id = payload.get("device_id").and_then(|v| v.as_str()).unwrap_or("unknown");
    let device_type_val = payload.get("device_type").and_then(|v| v.as_str()).unwrap_or("unknown");
    let device_model_val = payload.get("device_model").and_then(|v| v.as_str()).unwrap_or("unknown");
    let network_interfaces_val = payload.get("network_interfaces").and_then(|v| v.as_str());
    let ip_address_val = payload.get("ip_address").and_then(|v| v.as_str());

    let _ = diesel::insert_into(devices)
        .values((
            device_name.eq(device_id),
            device_type.eq(device_type_val),
            device_model.eq(device_model_val),
            network_interfaces.eq(network_interfaces_val),
            ip_address.eq(ip_address_val),
            approved.eq(true),
            last_checkin.eq(Utc::now().naive_utc()),
        ))
        .on_conflict(device_name)
        .do_update()
        .set((
            last_checkin.eq(Utc::now().naive_utc()),
            network_interfaces.eq(network_interfaces_val),
            ip_address.eq(ip_address_val),
        ))
        .execute(&mut conn);

    Json(json!({"adopted": true}))
}

#[get("/devices")]
async fn get_devices(state: &State<AppState>) -> Result<Json<Vec<Device>>, String> {
    use crate::schema::devices::dsl::*;
    let mut conn = establish_connection(&state.db_pool)?;
    let results = devices
        .load::<Device>(&mut conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|d| d.enrich_for_dashboard())
        .collect::<Vec<Device>>();
    Ok(Json(results))
}

#[get("/status")]
fn status(state: &State<AppState>) -> Json<serde_json::Value> {
    let mut sys = state.system.lock().unwrap();
    sys.refresh_all();

    Json(json!({
        "server_time": Utc::now().to_rfc3339(),
        "status": "ok",
        "uptime_seconds": sys.uptime(),
        "total_memory": sys.total_memory(),
        "used_memory": sys.used_memory(),
        "total_swap": sys.total_swap(),
        "used_swap": sys.used_swap(),
        "cpu_count": sys.cpus().len(),
        "cpu_usage_per_core": sys.cpus().iter().map(|c| c.cpu_usage()).collect::<Vec<f32>>(),
    }))
}

// --- Serve web UI ---
#[get("/")]
async fn dashboard() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/templates/dashboard.html").await.ok()
}

#[get("/favicon.ico")]
async fn favicon() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/static/favicon.ico").await.ok()
}

// --- DB initialization ---
fn initialize_db(conn: &mut SqliteConnection) -> Result<(), diesel::result::Error> {
    diesel::sql_query(r#"
        CREATE TABLE IF NOT EXISTS devices (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device_name TEXT NOT NULL UNIQUE,
            hostname TEXT NOT NULL,
            os_name TEXT NOT NULL,
            architecture TEXT NOT NULL,
            last_checkin TIMESTAMP NOT NULL,
            approved BOOLEAN NOT NULL,
            cpu FLOAT NOT NULL,
            ram_total BIGINT NOT NULL,
            ram_used BIGINT NOT NULL,
            ram_free BIGINT NOT NULL,
            disk_total BIGINT NOT NULL,
            disk_free BIGINT NOT NULL,
            disk_health TEXT NOT NULL,
            network_throughput BIGINT NOT NULL,
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

// --- Network helpers ---
fn get_server_ip() -> String {
    local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "127.0.0.1".to_string())
}

// --- Rocket server entry point ---
#[launch]
fn rocket() -> _ {
    init_logger();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = Pool::builder()
        .build(manager)
        .expect("Failed to create DB pool");

    {
        let mut conn = pool.get().expect("Failed to get DB connection for initialization");
        initialize_db(&mut conn).expect("Failed to initialize database schema");
        info!("✅ Database schema initialized or already exists");
    }

    let ip = get_server_ip();
    let port = 8080;
    info!("Server listening on 0.0.0.0, accessible on LAN at http://{}:{}/", ip, port);

    // Shared AppState with DB pool and System instance
    let state = AppState {
        db_pool: pool,
        system: std::sync::Mutex::new(System::new_all()),
    };

    rocket::build()
        .manage(state)
        .mount("/api", routes![register_or_update_device, get_devices, status, heartbeat])
        .mount("/", routes![dashboard, favicon])
        .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
}
