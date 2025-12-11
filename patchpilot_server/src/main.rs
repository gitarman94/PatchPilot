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

use crate::models::{
    Device, NewDevice, DeviceInfo, SystemInfo,
    Action, NewAction, ActionTarget, NewActionTarget,
    AuditRecord, NewAuditRecord,
};
use sysinfo::System;

mod settings;
use crate::settings::ServerSettings;

mod schema;
mod models;

use diesel::sqlite::SqliteConnection;
use crate::schema::{devices, actions as actions_table, action_targets, audit_log};

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

/// Sweeper: expire top-level actions and action targets when TTL passed.
/// Marks expired top-level actions (expires_at) and marks their targets as "expired".
pub fn spawn_action_ttl_sweeper(pool: DbPool) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;

            let pool = pool.clone();
            let _ = rocket::tokio::task::spawn_blocking(move || {
                use diesel::prelude::*;
                use crate::schema::actions::dsl as top_actions_dsl;
                use crate::schema::action_targets::dsl as targets_dsl;
                use crate::schema::audit_log::dsl as audit_dsl;

                let mut conn = match pool.get() {
                    Ok(c) => c,
                    Err(_) => return,
                };

                // Expire top-level actions whose expires_at <= now and are not already canceled.
                if let Ok(expired_actions) = top_actions_dsl::actions
                    .filter(top_actions_dsl::expires_at.le(Utc::now().naive_utc()))
                    .filter(top_actions_dsl::canceled.eq(false))
                    .load::<Action>(&mut conn)
                {
                    for act in expired_actions {
                        let details = serde_json::json!({
                            "action_type": act.action_type,
                            "parameters": act.parameters,
                            "reason": "expired"
                        }).to_string();

                        let audit = NewAuditRecord::new(Some(act.id.clone()), None, Some(act.author.clone().unwrap_or_else(|| "system".into())), "expired".into(), Some(details));
                        let _ = diesel::insert_into(audit_log::table).values(&audit).execute(&mut conn);

                        // mark action canceled/expired
                        let _ = diesel::update(top_actions_dsl::actions.filter(top_actions_dsl::id.eq(act.id.clone())))
                            .set(top_actions_dsl::canceled.eq(true))
                            .execute(&mut conn);

                        // mark related targets as expired when still pending
                        let _ = diesel::update(targets_dsl::action_targets.filter(targets_dsl::action_id.eq(act.id.clone())).filter(targets_dsl::status.eq("pending")))
                            .set((targets_dsl::status.eq("expired"), targets_dsl::last_update.eq(Utc::now().naive_utc())))
                            .execute(&mut conn);
                    }
                }
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

/// Insert or update a device using a stable UUID (device_uuid) and a friendly name.
/// If a row exists for the uuid, it will be updated; otherwise insert (if unique uuid index exists).
fn insert_or_update_device(
    conn: &mut SqliteConnection,
    device_uuid: &str,
    friendly_name: &str,
    info: &DeviceInfo
) -> Result<Device, ApiError> {
    use crate::schema::devices::dsl::*;

    // Find existing by uuid, otherwise try by device_name (legacy)
    let existing = devices
        .filter(uuid.eq(device_uuid))
        .first::<Device>(conn)
        .or_else(|_| devices.filter(device_name.eq(friendly_name)).first::<Device>(conn))
        .ok();

    let new_device = NewDevice::from_device_info(device_uuid, friendly_name, info, existing.as_ref());

    diesel::insert_into(devices)
        .values(&new_device)
        .on_conflict(uuid)
        .do_update()
        .set(&new_device)
        .execute(conn)
        .map_err(ApiError::from)?;

    let updated = devices
        .filter(uuid.eq(device_uuid))
        .first::<Device>(conn)
        .map_err(ApiError::from)?;

    Ok(updated.enrich_for_dashboard())
}

/// Add device to pending adoption list (in-memory)
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

/// CLIENT attempts DB update → only allowed if already approved.
/// This endpoint expects the device to use its stable UUID as the path parameter.
#[post("/devices/<device_id>", format = "json", data = "<device_info>")]
async fn register_or_update_device(
    pool: &State<DbPool>,
    device_id: &str,
    device_info: Json<DeviceInfo>,
) -> Result<Json<Device>, String> {

    validate_device_info(&device_info).map_err(|e| e.message())?;

    let pool = pool.inner().clone();
    // device_id here is expected to be stable device UUID
    let device_uuid = device_id.to_string();
    let device_info = device_info.into_inner();

    rocket::tokio::task::spawn_blocking(move || {
        use crate::schema::devices::dsl::*;
        let mut conn = establish_connection(&pool).map_err(|e| e.message())?;

        // ensure device exists and is approved
        let approved_state = devices
            .filter(uuid.eq(&device_uuid))
            .select(approved)
            .first::<bool>(&mut conn)
            .unwrap_or(false);

        if !approved_state {
            return Err(format!("Device {} not adopted (approved) yet", device_uuid));
        }

        // friendly name: prefer hostname in payload if available
        let friendly = device_info.system_info.ip_address.clone().unwrap_or(device_uuid.clone());

        insert_or_update_device(&mut conn, &device_uuid, &friendly, &device_info)
            .map(Json)
            .map_err(|e| e.message())
    })
    .await
    .unwrap_or_else(|e| Err(format!("Join error: {}", e)))
}

// HEARTBEAT — accepts complete JSON. UUID (device_id/device_uuid) is the stable identity.
// Returns: adoption status, pending actions, and client settings from server policy.
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

    // Extract preferred device UUID
    let device_uuid_opt = payload_value
        .get("device_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            payload_value
                .get("device_uuid")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

    // Extract friendly hostname for UI
    let friendly_name_opt = payload_value
        .get("system_info")
        .and_then(|si| si.get("hostname"))
        .and_then(|h| h.as_str())
        .map(|s| s.to_string());

    // Deserialize DeviceInfo, fallback-friendly if mismatched
    let mut incoming_info: DeviceInfo = match serde_json::from_value(payload_value.clone()) {
        Ok(di) => di,
        Err(_) => {
            let sys = payload_value
                .get("system_info")
                .and_then(|si| serde_json::from_value(si.clone()).ok())
                .unwrap_or(SystemInfo::default());

            DeviceInfo {
                device_uuid: device_uuid_opt.clone(),
                system_info: sys,
                device_type: payload_value
                    .get("device_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                device_model: payload_value
                    .get("device_model")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            }
        }
    };

    // Ensure proper UUID is filled
    if incoming_info.device_uuid.is_none() {
        if let Some(ref u) = device_uuid_opt {
            incoming_info.device_uuid = Some(u.clone());
        }
    }

    // Server settings (includes heartbeat rate)
    let settings_snapshot = {
        let cfg = state.settings.read().unwrap();
        cfg.clone()
    };

    let auto_approve = settings_snapshot.auto_approve_devices;

    let pending_ref = Arc::clone(&state.pending_devices);
    let incoming_clone = incoming_info.clone();
    let try_device_uuid = device_uuid_opt.clone();
    let try_friendly = friendly_name_opt
        .clone()
        .unwrap_or_else(|| try_device_uuid.clone().unwrap_or_default());

    let try_device_uuid_clone = try_device_uuid.clone();
    let adopted = rocket::tokio::task::spawn_blocking(move || {
        let try_device_uuid = try_device_uuid_clone;
        let mut conn = pool_clone.get().ok()?;

        if let Some(ref uuid_str) = try_device_uuid {
            let is_approved = devices
                .filter(uuid.eq(uuid_str))
                .select(approved)
                .first::<bool>(&mut conn)
                .unwrap_or(false);

            if is_approved {
                let existing = devices
                    .filter(uuid.eq(uuid_str))
                    .first::<Device>(&mut conn)
                    .ok();

                let newdev = NewDevice::from_device_info(
                    uuid_str,
                    &try_friendly,
                    &incoming_clone,
                    existing.as_ref(),
                );

                let _ = diesel::update(devices.filter(uuid.eq(uuid_str)))
                    .set(&newdev)
                    .execute(&mut conn)
                    .ok();

                return Some(true);
            }

            if auto_approve {
                let mut newdev = NewDevice::from_device_info(
                    uuid_str,
                    &try_friendly,
                    &incoming_clone,
                    None,
                );
                newdev.approved = true;
                newdev.last_checkin = Utc::now().naive_utc();

                let _ = diesel::insert_into(devices)
                    .values(&newdev)
                    .on_conflict(uuid)
                    .do_update()
                    .set(&newdev)
                    .execute(&mut conn);

                return Some(true);
            }
        }

        let mut pending = pending_ref.write().unwrap();
        let key = try_friendly.clone();
        pending
            .entry(key.clone())
            .and_modify(|existing| existing.merge_with(&incoming_clone))
            .or_insert(incoming_clone);

        Some(false)
    })
    .await
    .unwrap_or(Some(false))
    .unwrap_or(false);


    // Fetch pending actions
    let actions_for_device = {
        let pool_for_actions = pool.inner().clone();
        rocket::tokio::task::spawn_blocking(move || {
            use crate::schema::action_targets::dsl as targets_dsl;
            use crate::schema::actions::dsl as top_actions_dsl;
            use diesel::prelude::*;

            let mut conn = pool_for_actions.get().ok()?;
            let uuid_str = try_device_uuid?;

            let targets = targets_dsl::action_targets
                .filter(targets_dsl::device_uuid.eq(&uuid_str))
                .filter(targets_dsl::status.eq("pending"))
                .load::<ActionTarget>(&mut conn)
                .unwrap_or_default();

            let mut pairs = Vec::new();
            for t in targets {
                let parent = top_actions_dsl::actions
                    .filter(top_actions_dsl::id.eq(&t.action_id))
                    .first::<Action>(&mut conn)
                    .ok();
                pairs.push((t, parent));
            }

            Some(pairs)
        })
        .await
        .unwrap_or(None)
        .unwrap_or_default()
    };

    let actions_json: Vec<serde_json::Value> = actions_for_device
        .into_iter()
        .map(|(t, maybe_act)| {
            json!({
                "target_id": t.id,
                "action_id": t.action_id,
                "status": t.status,
                "last_update": t.last_update,
                "response": t.response,
                "action": maybe_act.map(|a| json!({
                    "id": a.id,
                    "action_type": a.action_type,
                    "parameters": a.parameters,
                    "author": a.author,
                    "created_at": a.created_at,
                    "expires_at": a.expires_at
                }))
            })
        })
        .collect();

    Json(json!({
        "adopted": adopted,
        "actions": actions_json,
        "settings": {
            "heartbeat_interval_sec": settings_snapshot.default_action_ttl_seconds
        }
    }))
}

/// CLIENT reports result for an action target
/// payload: { device_id: "<uuid>", status: "completed"|"failed", response: <string/json>, reported_by: "agent" }
#[post("/actions/<action_id>/report", format="json", data="<payload>")]
async fn report_action_result(pool: &State<DbPool>, action_id: &str, payload: Json<JsonValue>) -> Result<Json<serde_json::Value>, String> {
    use diesel::prelude::*;
    use crate::schema::action_targets::dsl as targets_dsl;

    let status_str = payload.get("status").and_then(|v| v.as_str()).unwrap_or("failed").to_string();
    let response_text = payload.get("response").map(|r| r.to_string());
    let device_id = payload.get("device_id").and_then(|v| v.as_str()).ok_or("missing device_id")?.to_string();
    let reported_by = payload.get("reported_by").and_then(|v| v.as_str()).map(|s| s.to_string());

    let pool = pool.inner().clone();
    let action_id = action_id.to_string();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;

        let updated = diesel::update(targets_dsl::action_targets.filter(targets_dsl::action_id.eq(&action_id)).filter(targets_dsl::device_uuid.eq(&device_id)))
            .set((
                targets_dsl::status.eq(&status_str),
                targets_dsl::response.eq(response_text.clone()),
                targets_dsl::last_update.eq(Utc::now().naive_utc()),
            ))
            .execute(&mut conn)
            .map_err(|e: diesel::result::Error| e.to_string())?;

        if updated == 0 {
            return Err(format!("no action target found for action {} and device {}", action_id, device_id));
        }

        let details = serde_json::json!({
            "device_id": device_id,
            "response": response_text,
            "status": status_str
        }).to_string();
        let audit = NewAuditRecord::new(Some(action_id.clone()), Some(device_id.clone()), reported_by.clone(), "completed".into(), Some(details));
        let _ = diesel::insert_into(audit_log::table).values(&audit).execute(&mut conn);

        Ok(Json(json!({ "status": "recorded" })))
    })
    .await
    .map_err(|e| format!("Join error: {}", e))?
}

/// ADMIN: list top-level actions with targets
#[get("/actions")]
async fn list_actions(pool: &State<DbPool>) -> Result<Json<serde_json::Value>, String> {
    use diesel::prelude::*;
    use crate::schema::actions::dsl as actions_dsl;
    use crate::schema::action_targets::dsl as targets_dsl;

    let pool = pool.inner().clone();
    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;

        let rows = actions_dsl::actions
            .order(actions_dsl::created_at.desc())
            .load::<Action>(&mut conn)
            .map_err(|e| e.to_string())?;

        let mut arr = Vec::new();
        for a in rows {
            let trows = targets_dsl::action_targets.filter(targets_dsl::action_id.eq(&a.id)).load::<ActionTarget>(&mut conn).unwrap_or_default();
            let targets_json: Vec<_> = trows.into_iter().map(|t| json!({
                "id": t.id,
                "device_uuid": t.device_uuid,
                "status": t.status,
                "last_update": t.last_update,
                "response": t.response
            })).collect();

            arr.push(json!({
                "id": a.id,
                "action_type": a.action_type,
                "parameters": a.parameters,
                "author": a.author,
                "created_at": a.created_at,
                "expires_at": a.expires_at,
                "canceled": a.canceled,
                "targets": targets_json
            }));
        }

        Ok(Json(json!({ "actions": arr })))
    })
    .await
    .map_err(|e| format!("Join error: {}", e))?
}

/// ADMIN: cancel a top-level action (mark canceled + audit, mark targets canceled)
#[post("/actions/<action_id>/cancel")]
async fn cancel_action(pool: &State<DbPool>, action_id: &str) -> Result<Json<serde_json::Value>, String> {
    use diesel::prelude::*;
    use crate::schema::actions::dsl as actions_dsl;
    use crate::schema::action_targets::dsl as targets_dsl;

    let pool = pool.inner().clone();
    let action_id = action_id.to_string();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;

        if let Ok(act) = actions_dsl::actions.filter(actions_dsl::id.eq(&action_id)).first::<Action>(&mut conn) {
            let _ = diesel::update(actions_dsl::actions.filter(actions_dsl::id.eq(&action_id))).set(actions_dsl::canceled.eq(true)).execute(&mut conn);

            let _ = diesel::update(targets_dsl::action_targets.filter(targets_dsl::action_id.eq(&action_id)).filter(targets_dsl::status.eq("pending")))
                .set((targets_dsl::status.eq("canceled"), targets_dsl::last_update.eq(Utc::now().naive_utc())))
                .execute(&mut conn);

            let details = serde_json::json!({
                "action_type": act.action_type,
                "parameters": act.parameters,
                "reason": "cancelled_by_admin"
            }).to_string();

            let audit = NewAuditRecord::new(Some(action_id.clone()), None, Some("admin".into()), "cancelled".into(), Some(details));
            let _ = diesel::insert_into(audit_log::table).values(&audit).execute(&mut conn);

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

    Json(json!({
        "auto_refresh_enabled": settings.auto_refresh_enabled
    }))
}

#[post("/settings/auto_refresh_interval/<seconds>")]
fn set_auto_refresh_interval(state: &State<AppState>, seconds: u64) -> Json<serde_json::Value> {
    let mut settings = state.settings.write().unwrap();

    settings.auto_refresh_seconds = if seconds == 0 { 30 } else { seconds };
    settings.save();

    Json(json!({
        "auto_refresh_seconds": settings.auto_refresh_seconds
    }))
}

/// Audit listing endpoint (used by UI history/audit page)
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
            .load::<AuditRecord>(&mut conn)
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
    // Devices table
    diesel::sql_query(r#"
        CREATE TABLE IF NOT EXISTS devices (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            uuid TEXT NOT NULL UNIQUE,
            device_name TEXT NOT NULL,
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

    // Backward-compat: add uuid column if missing (ignore errors)
    diesel::sql_query(r#"ALTER TABLE devices ADD COLUMN uuid TEXT;"#).execute(conn).ok();

    // Ensure unique index on uuid (so ON CONFLICT(uuid) works)
    diesel::sql_query(r#"CREATE UNIQUE INDEX IF NOT EXISTS idx_devices_uuid ON devices(uuid);"#).execute(conn)?;

    // Legacy device_actions kept for compatibility (optional)
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
    "#).execute(conn).ok();

    // High-level actions
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

    // Action targets: use device_uuid (text) to link to devices.uuid
    diesel::sql_query(r#"
        CREATE TABLE IF NOT EXISTS action_targets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            action_id TEXT NOT NULL,
            device_uuid TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            last_update TIMESTAMP NOT NULL,
            response TEXT
        );
    "#).execute(conn)?;

    // Audit log
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

#[get("/history.html")]
async fn history_page() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/templates/history.html")
        .await
        .ok()
}

// --- MISSING ROUTES ---
#[get("/devices")]
async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, String> {
    use crate::schema::devices::dsl::*;
    let pool = pool.inner().clone();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        let all_devices = devices.load::<Device>(&mut conn).map_err(|e| e.to_string())?;
        Ok(Json(all_devices))
    })
    .await
    .map_err(|e| format!("Join error: {}", e))?
}

#[get("/device_detail/<device_uuid>")]
async fn get_device_details(pool: &State<DbPool>, device_uuid: &str) -> Result<Json<Device>, String> {
    use crate::schema::devices::dsl::*;
    let pool = pool.inner().clone();
    let device_uuid = device_uuid.to_string();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        let dev = devices.filter(uuid.eq(&device_uuid)).first::<Device>(&mut conn).map_err(|e| e.to_string())?;
        Ok(Json(dev))
    })
    .await
    .map_err(|e| format!("Join error: {}", e))?
}

#[post("/approve/<device_uuid>")]
async fn approve_device(pool: &State<DbPool>, device_uuid: &str) -> Result<Json<serde_json::Value>, String> {
    use crate::schema::devices::dsl::*;
    let pool = pool.inner().clone();
    let device_uuid = device_uuid.to_string();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        diesel::update(devices.filter(uuid.eq(&device_uuid)))
            .set(approved.eq(true))
            .execute(&mut conn)
            .map_err(|e| e.to_string())?;

        Ok(Json(json!({ "status": "approved", "device_uuid": device_uuid })))
    })
    .await
    .map_err(|e| format!("Join error: {}", e))?
}

#[post("/submit_action", format="json", data="<action>")]
async fn submit_action(pool: &State<DbPool>, action: Json<NewAction>) -> Result<Json<serde_json::Value>, String> {
    use crate::schema::actions::dsl::actions as actions_dsl;
    let pool = pool.inner().clone();
    let action = action.into_inner();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e: diesel::result::Error| e.to_string())?;
        diesel::insert_into(actions_dsl)
            .values(&action)
            .execute(&mut conn)
            .map_err(|e: diesel::result::Error| e.to_string())?;
        Ok(Json(json!({ "status": "submitted", "action_id": action.id })))
    })
    .await
    .map_err(|e| format!("Join error: {}", e))?
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
                get_devices,
                get_device_details,
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
        .mount("/", routes![dashboard, device_detail, favicon, actions_page, audit_page, history_page])
}
