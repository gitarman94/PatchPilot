use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use r2d2::Pool;
use rocket::{get, post, routes, launch, State};
use rocket::serde::json::Json;
use rocket::fs::{FileServer, NamedFile};
use flexi_logger::{Logger, FileSpec, Age, Cleanup, Criterion, Naming};
use log::info;
use serde_json::{json, Value as JsonValue};
use chrono::Utc;
use local_ip_address::local_ip;
use std::sync::{Mutex, Arc, RwLock};
use std::collections::HashMap;
use uuid::Uuid;

use crate::models::{Device, NewDevice, DeviceInfo, SystemInfo};
use sysinfo::System;

mod settings;
use crate::settings::ServerSettings;

mod schema;
mod models;

use diesel::sqlite::SqliteConnection;

type DbPool = Pool<ConnectionManager<SqliteConnection>>;

pub struct AppState {
    pub system: Mutex<System>,
    pub pending_devices: Arc<RwLock<HashMap<String, DeviceInfo>>>,
    pub settings: Arc<RwLock<ServerSettings>>,
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

fn establish_connection(pool: &DbPool)
    -> Result<PooledConnection<ConnectionManager<SqliteConnection>>, ApiError> {
    pool.get()
        .map_err(|e| ApiError::ValidationError(format!("Failed to get DB connection: {}", e)))
}

fn validate_device_info(info: &DeviceInfo) -> Result<(), ApiError> {
    if info.system_info.cpu_usage < 0.0 {
        return Err(ApiError::ValidationError("CPU usage cannot be negative".into()));
    }
    if info.system_info.ram_total <= 0 {
        return Err(ApiError::ValidationError("RAM total must be positive".into()));
    }
    Ok(())
}

// DB upsert for approved devices only
fn insert_or_update_device(
    conn: &mut SqliteConnection,
    device_id: &str,
    info: &DeviceInfo
) -> Result<Device, ApiError> {
    use crate::schema::devices::dsl::*;

    // Pull existing dev if present
    let existing = devices
        .filter(device_name.eq(device_id))
        .first::<Device>(conn)
        .ok();

    let new_device = NewDevice::from_device_info(device_id, info, existing.as_ref());

    diesel::insert_into(devices)
        .values(&new_device)
        .on_conflict(device_name)
        .do_update()
        .set(&new_device)
        .execute(conn)
        .map_err(ApiError::from)?;

    let updated = devices
        .filter(device_name.eq(device_id))
        .first::<Device>(conn)
        .map_err(ApiError::from)?;

    Ok(updated.enrich_for_dashboard())
}

// Adds device to PENDING ONLY (in-memory)
#[post("/register", format = "json", data = "<payload>")]
async fn register_device(
    state: &State<AppState>,
    payload: Json<DeviceInfo>,
) -> Result<Json<serde_json::Value>, String>
{
    let pending_id = format!("pending-{}", Uuid::new_v4());

    let mut pending = state.pending_devices.write().unwrap();
    pending.insert(pending_id.clone(), payload.into_inner());

    Ok(Json(json!({
        "pending_id": pending_id,
        "status": "pending_adoption"
    })))
}

// CLIENT attempts DB update → only allowed if already approved
#[post("/devices/<device_id>", format = "json", data = "<device_info>")]
async fn register_or_update_device(
    pool: &State<DbPool>,
    device_id: &str,
    device_info: Json<DeviceInfo>,
) -> Result<Json<Device>, String> {

    validate_device_info(&device_info).map_err(|e| e.message())?;

    let pool = pool.inner().clone();
    let device_id = device_id.to_string();
    let device_info = device_info.into_inner();

    rocket::tokio::task::spawn_blocking(move || {
        use crate::schema::devices::dsl::*;
        let mut conn = establish_connection(&pool).map_err(|e| e.message())?;

        let approved_state = devices
            .filter(device_name.eq(&device_id))
            .select(approved)
            .first::<bool>(&mut conn)
            .unwrap_or(false);

        if !approved_state {
            return Err(format!("Device {} not adopted (approved) yet", device_id));
        }

        insert_or_update_device(&mut conn, &device_id, &device_info)
            .map(Json)
            .map_err(|e| e.message())
    })
    .await
    .unwrap_or_else(|e| Err(format!("Join error: {}", e)))
}

// Heartbeat → approved devices update DB; unapproved stay in pending only
#[post("/devices/heartbeat", format="json", data="<payload>")]
async fn heartbeat(
    state: &State<AppState>,
    pool: &State<DbPool>,
    payload: Json<JsonValue>,
) -> Json<serde_json::Value>
{
    use crate::schema::devices::dsl::*;

    let pool_clone = pool.inner().clone();
    let payload_value = payload.into_inner();

    // device_id can be provided as top-level "device_id" or inside system_info.hostname
    let device_id = payload_value.get("device_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            payload_value.get("system_info")
                .and_then(|si| si.get("hostname"))
                .and_then(|h| h.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    if device_id.is_empty() {
        return Json(json!({ "adopted": false, "error": "missing device_id or system_info.hostname" }));
    }

    // Try to deserialize DeviceInfo from the payload; fallback to minimal DeviceInfo
    let incoming_info: DeviceInfo = match serde_json::from_value(payload_value.clone()) {
        Ok(di) => di,
        Err(_) => {
            // build minimal DeviceInfo from available fields
            let sys = payload_value.get("system_info").and_then(|si| serde_json::from_value(si.clone()).ok()).unwrap_or(SystemInfo::default());
            let dtype = payload_value.get("device_type").and_then(|v| v.as_str()).map(|s| s.to_string());
            let dmodel = payload_value.get("device_model").and_then(|v| v.as_str()).map(|s| s.to_string());
            DeviceInfo {
                system_info: sys,
                device_type: dtype,
                device_model: dmodel,
            }
        }
    };

    let auto_approve = {
        let cfg = state.settings.read().unwrap();
        cfg.auto_approve_devices
    };

    let pending_ref = Arc::clone(&state.pending_devices);
    let incoming_clone = incoming_info.clone();

    let adopted = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = match pool_clone.get() {
            Ok(c) => c,
            Err(_) => return false,
        };

        let is_approved = devices
            .filter(device_name.eq(&device_id))
            .select(approved)
            .first::<bool>(&mut conn)
            .unwrap_or(false);

        if is_approved {
            let existing = devices
                .filter(device_name.eq(&device_id))
                .first::<Device>(&mut conn)
                .ok();

            let newdev = NewDevice::from_device_info(&device_id, &incoming_clone, existing.as_ref());

            let _ = diesel::update(devices.filter(device_name.eq(&device_id)))
                .set(&newdev)
                .execute(&mut conn)
                .ok();

            return true;
        }

        if auto_approve {
            let mut newdev = NewDevice::from_device_info(&device_id, &incoming_clone, None);
            newdev.approved = true;

            let _ = diesel::insert_into(devices)
                .values(&newdev)
                .on_conflict(device_name)
                .do_update()
                .set(&newdev)
                .execute(&mut conn);

            return true;
        }

        let mut pending = pending_ref.write().unwrap();
        pending
            .entry(device_id.clone())
            .and_modify(|existing| existing.merge_with(&incoming_clone))
            .or_insert(incoming_clone);

        false
    })
    .await
    .unwrap_or(false);

    Json(json!({ "adopted": adopted }))
}

#[post("/settings/auto_approve/<enable>")]
fn set_auto_approve(state: &State<AppState>, enable: bool) -> Json<serde_json::Value> {
    let mut settings = state.settings.write().unwrap();
    settings.auto_approve_devices = enable;
    settings.save();
    Json(json!({ "auto_approve": enable }))
}

// Get one device (approved only)
#[get("/device/<device_id>")]
async fn get_device_details(
    pool: &State<DbPool>,
    device_id: String,
) -> Result<Json<Device>, String> {
    use crate::schema::devices::dsl::*;

    let pool = pool.inner().clone();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;

        let device = devices
            .filter(device_name.eq(&device_id))
            .first::<Device>(&mut conn)
            .map_err(|e| e.to_string())?;

        Ok(Json(device.enrich_for_dashboard()))
    })
    .await
    .unwrap_or_else(|e| Err(format!("Join error: {}", e)))
}

#[get("/status")]
fn status(state: &State<AppState>) -> Json<serde_json::Value> {
    let mut sys = state.system.lock().unwrap();
    sys.refresh_all();

    Json(json!({
        "status": "ok",
        "server_time": Utc::now().to_rfc3339(),
        "uptime_seconds": System::uptime(),
        "cpu_count": sys.cpus().len(),
        "cpu_usage_per_core_percent": sys.cpus().iter().map(|c| c.cpu_usage()).collect::<Vec<f32>>(),
        "total_memory_bytes": sys.total_memory(),
        "used_memory_bytes": sys.used_memory(),
        "memory_usage_percent": if sys.total_memory() > 0 {
            (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0
        } else { 0.0 },
    }))
}

// Return all devices (approved from DB + pending in-memory)
#[get("/devices")]
async fn get_devices(pool: &State<DbPool>, state: &State<AppState>) -> Result<Json<Vec<serde_json::Value>>, String> {
    use crate::schema::devices::dsl::*;

    let pool = pool.inner().clone();
    let pending_ref = Arc::clone(&state.pending_devices);

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;

        let mut list: Vec<serde_json::Value> = devices
            .load::<Device>(&mut conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|d| json!({
                "id": d.id.to_string(),
                "device_name": d.device_name,
                "hostname": d.hostname,
                "os_name": d.os_name,
                "architecture": d.architecture,
                "cpu_usage": d.cpu_usage,
                "cpu_count": d.cpu_count,
                "cpu_brand": d.cpu_brand,
                "ram_total": d.ram_total,
                "ram_used": d.ram_used,
                "disk_total": d.disk_total,
                "disk_free": d.disk_free,
                "disk_health": d.disk_health,
                "network_throughput": d.network_throughput,
                "ping_latency": d.ping_latency,
                "ip_address": d.ip_address,
                "network_interfaces": d.network_interfaces,
                "uptime": d.uptime,
                "updates_available": d.updates_available,
                "approved": d.approved,
                "pending": false,
                "last_checkin": d.last_checkin
            }))
            .collect();

        let pending = pending_ref.read().unwrap();
        for (pending_id, p) in pending.iter() {
            list.push(json!({
                "id": pending_id,
                "device_name": "", // name not provided for pending DeviceInfo
                "hostname": pending_id,
                "os_name": p.system_info.os_name,
                "architecture": p.system_info.architecture,
                "cpu_usage": p.system_info.cpu_usage,
                "cpu_count": p.system_info.cpu_count,
                "cpu_brand": p.system_info.cpu_brand,
                "ram_total": p.system_info.ram_total,
                "ram_used": p.system_info.ram_used,
                "disk_total": p.system_info.disk_total,
                "disk_free": p.system_info.disk_free,
                "disk_health": p.system_info.disk_health,
                "network_throughput": p.system_info.network_throughput,
                "ping_latency": p.system_info.ping_latency,
                "ip_address": p.system_info.ip_address,
                "network_interfaces": p.system_info.network_interfaces,
                "uptime": serde_json::Value::Null,
                "updates_available": false,
                "approved": false,
                "pending": true,
                "last_checkin": "pending"
            }));
        }

        Ok(Json(list))
    })
    .await
    .unwrap_or_else(|e| Err(format!("Join error: {}", e)))
}

// Approve → pending → DB as approved
#[post("/devices/<device_id>/approve")]
async fn approve_device(
    state: &State<AppState>,
    pool: &State<DbPool>,
    device_id: String,
) -> Result<Json<Device>, String> {

    use crate::schema::devices::dsl::*;

    let pool = pool.inner().clone();
    let pending_ref = Arc::clone(&state.pending_devices);
    let dev_id = device_id.clone();

    rocket::tokio::task::spawn_blocking(move || {
        let mut pending = pending_ref.write().map_err(|e| format!("Lock error: {}", e))?;
        let info = pending
            .remove(&dev_id)
            .ok_or_else(|| format!("No pending device {}", dev_id))?;

        let mut conn = pool.get().map_err(|e| format!("DB pool error: {}", e))?;

        let mut new_dev = NewDevice::from_device_info(&dev_id, &info, None);
        new_dev.approved = true;

        diesel::insert_into(devices)
            .values(&new_dev)
            .on_conflict(device_name)
            .do_update()
            .set(&new_dev)
            .execute(&mut conn)
            .map_err(|e| format!("DB insert error: {}", e))?;

        let inserted = devices
            .filter(device_name.eq(&dev_id))
            .first::<Device>(&mut conn)
            .map_err(|e| format!("DB fetch error: {}", e))?;

        Ok(Json(inserted.enrich_for_dashboard()))
    })
    .await
    .map_err(|e| format!("Join error: {}", e))?
}

#[get("/")]
async fn dashboard() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/templates/dashboard.html")
        .await
        .ok()
}

#[get("/favicon.ico")]
async fn favicon() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/static/favicon.ico")
        .await
        .ok()
}

// Enable / disable auto-refresh (dashboard front-end behavior)
#[post("/settings/auto_refresh/<enable>")]
fn set_auto_refresh(state: &State<AppState>, enable: bool) -> Json<serde_json::Value> {
    let mut settings = state.settings.write().unwrap();
    settings.auto_refresh_enabled = enable;
    settings.save();

    Json(json!({
        "auto_refresh_enabled": settings.auto_refresh_enabled
    }))
}

// Set the auto-refresh interval (in seconds)
#[post("/settings/auto_refresh_interval/<seconds>")]
fn set_auto_refresh_interval(state: &State<AppState>, seconds: u64) -> Json<serde_json::Value> {
    let mut settings = state.settings.write().unwrap();

    // Fail-safe default if someone sends "0"
    settings.auto_refresh_seconds = if seconds == 0 { 30 } else { seconds };
    settings.save();

    Json(json!({
        "auto_refresh_seconds": settings.auto_refresh_seconds
    }))
}

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

            cpu_usage FLOAT NOT NULL DEFAULT 0.0,
            cpu_count INTEGER NOT NULL DEFAULT 0,
            cpu_brand TEXT NOT NULL DEFAULT '',

            ram_total BIGINT NOT NULL DEFAULT 0,
            ram_used BIGINT NOT NULL DEFAULT 0,

            disk_total BIGINT NOT NULL DEFAULT 0,
            disk_free BIGINT NOT NULL DEFAULT 0,
            disk_health TEXT NOT NULL DEFAULT '',

            network_throughput BIGINT NOT NULL DEFAULT 0,
            ping_latency FLOAT,

            device_type TEXT NOT NULL,
            device_model TEXT NOT NULL,

            uptime TEXT,
            updates_available BOOLEAN NOT NULL DEFAULT 0,

            network_interfaces TEXT,
            ip_address TEXT
        );
    "#).execute(conn)?;

    Ok(())
}

// Helper – determine server local IP
fn get_server_ip() -> String {
    local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "127.0.0.1".into())
}

#[get("/device_detail.html")]
async fn device_detail() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/templates/device_detail.html")
        .await
        .ok()
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
            pending_devices: Arc::new(RwLock::new(HashMap::new())),
            settings: Arc::new(RwLock::new(ServerSettings::load())),
        })
        .mount(
            "/api",
            routes![
                register_device,
                register_or_update_device,
                get_devices,
                get_device_details,
                status,
                heartbeat,
                approve_device,
                set_auto_approve,
                set_auto_refresh,
                set_auto_refresh_interval
            ],
        )
        .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
        .mount("/", routes![dashboard, device_detail, favicon])
}
