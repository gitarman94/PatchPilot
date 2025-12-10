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

use crate::models::{Device, NewDevice, DeviceInfo, SystemInfo, DeviceAction, NewDeviceAction, NewAuditLog};
use sysinfo::System;

mod settings;
use crate::settings::ServerSettings;

mod schema;
mod models;

use diesel::sqlite::SqliteConnection;
// keep device_actions import for backward compatibility
use crate::schema::{devices, device_actions, audit_log, actions as actions_table, action_targets};

type DbPool = Pool<ConnectionManager<SqliteConnection>>;

pub struct AppState {
    pub system: Mutex<System>,
    pub pending_devices: Arc<RwLock<HashMap<String, DeviceInfo>>>,
    pub settings: Arc<RwLock<ServerSettings>>,
}

use std::time::{Instant, Duration};
use rocket::tokio;

pub fn spawn_pending_cleanup(state: Arc<AppState>) {
    tokio::spawn(async move {
        let check_every = Duration::from_secs(5);
        let max_age = Duration::from_secs(15);

        let mut last_seen: HashMap<String, Instant> = HashMap::new();

        loop {
            tokio::time::sleep(check_every).await;
            let now = Instant::now();

            let mut pending = state.pending_devices.write().unwrap();
            for id in pending.keys() {
                last_seen.insert(id.clone(), now);
            }

            pending.retain(|id, _| {
                if let Some(t) = last_seen.get(id) {
                    now.duration_since(*t) < max_age
                } else {
                    false
                }
            });

            last_seen.retain(|id, _| pending.contains_key(id));
        }
    });
}

pub fn spawn_action_ttl_sweeper(pool: DbPool) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;

            let pool = pool.clone();
            let _ = rocket::tokio::task::spawn_blocking(move || {
                use diesel::dsl::*;
                use crate::schema::device_actions::dsl as actions_dsl;
                use crate::schema::audit_log::dsl as audit_dsl;
                // also check high-level actions table (if present)
                use crate::schema::actions::dsl as top_actions_dsl;
                use crate::schema::action_targets::dsl as targets_dsl;

                let mut conn = match pool.get() {
                    Ok(c) => c,
                    Err(_) => return,
                };

                // Sweep device_actions (existing per-device actions)
                let expired_rows = actions_dsl::device_actions
                    .filter(actions_dsl::status.eq("pending"))
                    .load::<DeviceAction>(&mut conn)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|a| {
                        let created = a.created_at;
                        let ttl = chrono::Duration::seconds(a.ttl_seconds as i64);
                        Utc::now().naive_utc() - created >= ttl
                    })
                    .collect::<Vec<_>>();

                for act in expired_rows {
                    let details = serde_json::json!({
                        "command": act.command,
                        "params": act.params,
                        "reason": "ttl_expired"
                    })
                    .to_string();

                    let new_audit = NewAuditLog::new(Some(act.id.clone()), Some(act.device_name.clone()), act.requested_by.clone(), "expired".into(), Some(details));

                    let _ = diesel::insert_into(audit_log::table)
                        .values(&new_audit)
                        .execute(&mut conn);

                    let _ = diesel::delete(device_actions::table.filter(device_actions::id.eq(act.id))).execute(&mut conn);
                }

                // Sweep high-level action_targets (mark expired targets)
                if let Ok(targets) = targets_dsl::action_targets.filter(targets_dsl::status.eq("pending")).load::<(i32, String, i32, String, chrono::NaiveDateTime, Option<String>)>(&mut conn) {
                    // NOTE: the tuple type above is a convenience; in a full implementation you'd have a model struct
                    // For now, just attempt a simple TTL logic if created/last_update info included in details; this is placeholder
                    // Full migration requires clearer timestamp fields on action_targets to compute expiration times.
                }

                // Sweep high-level actions table for expired top-level actions (placeholder)
                let _ = top_actions_dsl::actions.limit(0).load::<(String, String, Option<String>, Option<String>, chrono::NaiveDateTime, chrono::NaiveDateTime, bool)>(&mut conn);
            }).await;
        }
    });
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

fn insert_or_update_device(
    conn: &mut SqliteConnection,
    device_id: &str,
    info: &DeviceInfo
) -> Result<Device, ApiError> {
    use crate::schema::devices::dsl::*;

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

#[post("/devices/heartbeat", format="json", data="<payload>")]
async fn heartbeat(
    state: &State<AppState>,
    pool: &State<DbPool>,
    payload: Json<JsonValue>,
) -> Json<serde_json::Value>
{
    use crate::schema::devices::dsl::*;
    use crate::schema::device_actions::dsl as actions_dsl;

    let pool_clone = pool.inner().clone();
    let payload_value = payload.into_inner();

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

    let incoming_info: DeviceInfo = match serde_json::from_value(payload_value.clone()) {
        Ok(di) => di,
        Err(_) => {
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
    let pool_for_actions = pool_clone.clone();
    let device_id_clone = device_id.clone();

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

            newdev.last_checkin = Utc::now().naive_utc();

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

    let actions_for_device = rocket::tokio::task::spawn_blocking(move || {
        use diesel::prelude::*;
        use crate::schema::device_actions::dsl::*;

        let mut conn = match pool_for_actions.get() {
            Ok(c) => c,
            Err(_) => return Vec::<DeviceAction>::new(),
        };

        let mut items = device_actions
            .filter(device_name.eq(&device_id_clone))
            .filter(status.eq("pending"))
            .load::<DeviceAction>(&mut conn)
            .unwrap_or_default();

        items.retain(|a| {
            let created = a.created_at;
            let ttl = chrono::Duration::seconds(a.ttl_seconds as i64);
            Utc::now().naive_utc() - created < ttl
        });

        items
    })
    .await
    .unwrap_or_default();

    let actions_json: Vec<serde_json::Value> = actions_for_device
        .into_iter()
        .map(|a| {
            json!({
                "id": a.id,
                "command": a.command,
                "params": a.params,
                "ttl_seconds": a.ttl_seconds,
                "requested_by": a.requested_by,
                "created_at": a.created_at,
            })
        })
        .collect();

    Json(json!({
        "adopted": adopted,
        "actions": actions_json
    }))
}

/// ADMIN: submit action for a device (or wildcard device_name = "*")
#[post("/actions/submit", format = "json", data = "<payload>")]
async fn submit_action(pool: &State<DbPool>, payload: Json<JsonValue>) -> Result<Json<serde_json::Value>, String> {
    use diesel::prelude::*;
    use crate::schema::device_actions::dsl::*;

    let j = payload.into_inner();

    let device = j.get("device_name").and_then(|v| v.as_str()).ok_or("missing device_name")?.to_string();
    let cmd = j.get("command").and_then(|v| v.as_str()).ok_or("missing command")?.to_string();
    let params = j.get("params").map(|p| p.to_string());
    let ttl = j.get("ttl_seconds").and_then(|v| v.as_i64()).unwrap_or(3600) as i32;
    let requested_by = j.get("requested_by").and_then(|v| v.as_str()).map(|s| s.to_string());

    let new_id = Uuid::new_v4().to_string();
    let new_action = NewDeviceAction::new_pending(new_id.clone(), device.clone(), cmd.clone(), params.clone(), ttl, requested_by.clone());

    let pool = pool.inner().clone();
    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        diesel::insert_into(device_actions::table)
            .values(&new_action)
            .execute(&mut conn)
            .map_err(|e| e.to_string())?;

        let details = serde_json::json!({
            "command": cmd,
            "params": params,
            "ttl_seconds": ttl
        }).to_string();

        let audit = NewAuditLog::new(Some(new_id.clone()), Some(device.clone()), requested_by.clone(), "submitted".into(), Some(details));
        let _ = diesel::insert_into(audit_log::table).values(&audit).execute(&mut conn);

        Ok(Json(json!({ "action_id": new_id, "status": "queued" })))
    })
    .await
    .unwrap_or_else(|e| Err(format!("Join error: {}", e)))
}

/// CLIENT reports result for an action
#[post("/actions/<action_id>/report", format="json", data="<payload>")]
async fn report_action_result(pool: &State<DbPool>, action_id: &str, payload: Json<JsonValue>) -> Result<Json<serde_json::Value>, String> {
    use diesel::prelude::*;
    use crate::schema::device_actions::dsl as actions_dsl;

    let status_str = payload.get("status").and_then(|v| v.as_str()).unwrap_or("failed").to_string();
    let result_text = payload.get("result").map(|r| r.to_string());

    let pool = pool.inner().clone();
    let action_id = action_id.to_string();
    let res = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;

        if let Ok(act) = actions_dsl::device_actions.filter(actions_dsl::id.eq(&action_id)).first::<DeviceAction>(&mut conn) {
            let details = serde_json::json!({
                "command": act.command,
                "params": act.params,
                "result": result_text.clone(),
                "status": status_str.clone()
            }).to_string();

            let audit = NewAuditLog::new(Some(action_id.clone()), Some(act.device_name.clone()), payload.get("reported_by").and_then(|v| v.as_str()).map(|s| s.to_string()), "completed".into(), Some(details));
            let _ = diesel::insert_into(audit_log::table).values(&audit).execute(&mut conn);

            let _ = diesel::delete(actions_dsl::device_actions.filter(actions_dsl::id.eq(&action_id))).execute(&mut conn);

            Ok(Json(json!({ "status": "recorded" })))
        } else {
            Err(format!("action {} not found", action_id))
        }
    })
    .await
    .map_err(|e| format!("Join error: {}", e))?;

    res
}

/// ADMIN: list current pending/running actions (device_actions)
#[get("/actions")]
async fn list_actions(pool: &State<DbPool>) -> Result<Json<serde_json::Value>, String> {
    use diesel::prelude::*;
    use crate::schema::device_actions::dsl::*;

    let pool = pool.inner().clone();
    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        let rows = device_actions
            .order(created_at.desc())
            .load::<DeviceAction>(&mut conn)
            .map_err(|e| e.to_string())?;

        let arr: Vec<serde_json::Value> = rows.into_iter().map(|a| {
            json!({
                "id": a.id,
                "device_name": a.device_name,
                "command": a.command,
                "params": a.params,
                "ttl_seconds": a.ttl_seconds,
                "requested_by": a.requested_by,
                "status": a.status,
                "created_at": a.created_at,
                "updated_at": a.updated_at,
            })
        }).collect();

        Ok(Json(json!({ "actions": arr })))
    })
    .await
    .map_err(|e| format!("Join error: {}", e))?
}

/// ADMIN: cancel an action (move to audit & delete)
#[post("/actions/<action_id>/cancel")]
async fn cancel_action(pool: &State<DbPool>, action_id: &str) -> Result<Json<serde_json::Value>, String> {
    use diesel::prelude::*;
    use crate::schema::device_actions::dsl as actions_dsl;

    let pool = pool.inner().clone();
    let action_id = action_id.to_string();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;

        if let Ok(act) = actions_dsl::device_actions.filter(actions_dsl::id.eq(&action_id)).first::<DeviceAction>(&mut conn) {
            let details = serde_json::json!({
                "command": act.command,
                "params": act.params,
                "reason": "cancelled_by_admin"
            }).to_string();

            let audit = NewAuditLog::new(Some(action_id.clone()), Some(act.device_name.clone()), Some("admin".into()), "cancelled".into(), Some(details));
            let _ = diesel::insert_into(audit_log::table).values(&audit).execute(&mut conn);
            let _ = diesel::delete(actions_dsl::device_actions.filter(actions_dsl::id.eq(&action_id))).execute(&mut conn);

            Ok(Json(json!({ "status": "cancelled" })))
        } else {
            Err(format!("action {} not found", action_id))
        }
    })
    .await
    .map_err(|e| format!("Join error: {}", e))?
}

#[post("/settings/auto_approve/<enable>")]
fn set_auto_approve(state: &State<AppState>, enable: bool) -> Json<serde_json::Value> {
    let mut settings = state.settings.write().unwrap();
    settings.auto_approve_devices = enable;
    settings.save();
    Json(json!({ "auto_approve": enable }))
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

#[post("/settings/auto_refresh/<enable>")]
fn set_auto_refresh(state: &State<AppState>, enable: bool) -> Json<serde_json::Value> {
    let mut settings = state.settings.write().unwrap();
    settings.auto_refresh_enabled = enable;
    settings.save();

    Json(json!( {
        "auto_refresh_enabled": settings.auto_refresh_enabled
    }))
}

#[post("/settings/auto_refresh_interval/<seconds>")]
fn set_auto_refresh_interval(state: &State<AppState>, seconds: u64) -> Json<serde_json::Value> {
    let mut settings = state.settings.write().unwrap();

    settings.auto_refresh_seconds = if seconds == 0 { 30 } else { seconds };
    settings.save();

    Json(json!( {
        "auto_refresh_seconds": settings.auto_refresh_seconds
    }))
}

/// Simple audit listing endpoint (used by UI history/audit page)
#[get("/audit")]
async fn get_audit(pool: &State<DbPool>) -> Result<Json<serde_json::Value>, String> {
    use diesel::prelude::*;
    use crate::schema::audit_log::dsl::*;

    let pool = pool.inner().clone();
    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        let rows = audit_log
            .order(created_at.desc())
            .limit(500)
            .load::<crate::models::AuditLog>(&mut conn)
            .map_err(|e| e.to_string())?;

        let arr: Vec<serde_json::Value> = rows.into_iter().map(|r| {
            json!({
                "id": r.id,
                "action_id": r.action_id,
                "device_name": r.device_name,
                "actor": r.actor,
                "action_type": r.action_type,
                "details": r.details,
                "created_at": r.created_at,
            })
        }).collect();

        Ok(Json(json!({ "audit": arr })))
    })
    .await
    .map_err(|e| format!("Join error: {}", e))?
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

    // legacy per-device actions (kept for now)
    diesel::sql_query(r#"
        CREATE TABLE IF NOT EXISTS device_actions (
            id TEXT PRIMARY KEY,
            device_name TEXT NOT NULL,
            command TEXT NOT NULL,
            params TEXT,
            ttl_seconds INTEGER NOT NULL DEFAULT 3600,
            requested_by TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL,
            result TEXT
        );
    "#).execute(conn)?;

    // high-level actions table (one row per logical action)
    diesel::sql_query(r#"
        CREATE TABLE IF NOT EXISTS actions (
            id TEXT PRIMARY KEY,
            action_type TEXT NOT NULL,
            parameters TEXT,
            author TEXT,
            created_at TIMESTAMP NOT NULL,
            expires_at TIMESTAMP NOT NULL,
            canceled BOOLEAN NOT NULL DEFAULT 0
        );
    "#).execute(conn)?;

    // mapping table: which devices are targeted by an action
    diesel::sql_query(r#"
        CREATE TABLE IF NOT EXISTS action_targets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            action_id TEXT NOT NULL,
            device_id INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            last_update TIMESTAMP NOT NULL,
            response TEXT
        );
    "#).execute(conn)?;

    // audit_log table
    diesel::sql_query(r#"
        CREATE TABLE IF NOT EXISTS audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            action_id TEXT,
            device_name TEXT,
            actor TEXT,
            action_type TEXT NOT NULL,
            details TEXT,
            created_at TIMESTAMP NOT NULL
        );
    "#).execute(conn)?;

    Ok(())
}

fn get_server_ip() -> String {
    local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "127.0.0.1".into())
}

#[get("/device_detail.html")]
async fn device_detail() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/templates/device_detail.html")
        .await
        .ok()
}

#[get("/actions.html")]
async fn actions_page() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/templates/actions.html")
        .await
        .ok()
}

#[get("/audit_log.html")]
async fn audit_page() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/templates/audit_log.html")
        .await
        .ok()
}

#[launch]
fn rocket() -> _ {
    init_logger();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<SqliteConnection>::new(database_url.clone());
    let pool = Pool::builder().build(manager).expect("Failed to create DB pool");

    {
        let mut conn = pool.get().expect("DB connect failed");
        initialize_db(&mut conn).expect("DB init failed");
        info!("Database ready");
    }

    spawn_action_ttl_sweeper(pool.clone());

    let state_for_pending = Arc::new(AppState {
        system: Mutex::new(System::new_all()),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings: Arc::new(RwLock::new(ServerSettings::load())),
    });
    spawn_pending_cleanup(state_for_pending.clone());

    let ip = get_server_ip();
    let port = 8080;
    info!("Server running at http://{}:{}/", ip, port);

    let config = rocket::Config {
        address: "0.0.0.0".parse().unwrap(),
        port: 8080,
        ..Default::default()
    };

    rocket::custom(config)
        .manage(pool.clone())
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
                // NOTE: these route handlers (get_devices, get_device_details, status, approve_device, etc.)
                // must exist elsewhere in your codebase; keep them mounted.
                get_devices,
                get_device_details,
                status,
                heartbeat,
                approve_device,
                set_auto_approve,
                set_auto_refresh,
                set_auto_refresh_interval,
                submit_action,
                report_action_result,
                list_actions,
                cancel_action,
                get_audit
            ],
        )
        .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
        .mount("/", routes![dashboard, device_detail, favicon, actions_page, audit_page])
}
