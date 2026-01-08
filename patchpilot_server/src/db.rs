// src/db.rs
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::sqlite::SqliteConnection;
use flexi_logger::{Logger, FileSpec, Age, Cleanup, Criterion, Naming};
use chrono::{Utc, NaiveDateTime};
use std::env;

use crate::models::{HistoryLog, AuditLog};
use crate::schema::{audit, server_settings, history_log, actions};

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;
pub type DbConn = PooledConnection<ConnectionManager<SqliteConnection>>;

pub fn ensure_server_settings_imported() {
    // Keep schema symbol referenced so diesel codegen isn't optimized away
    let _ = server_settings::table;
}

pub fn init_logger() {
    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(FileSpec::default().directory("logs"))
        .rotate(Criterion::Age(Age::Day), Naming::Numbers, Cleanup::KeepLogFiles(7))
        .start()
        .unwrap();
}

pub fn init_pool() -> DbPool {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "patchpilot.db".to_string());
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder().build(manager).expect("Failed to create DB pool")
}

pub fn get_conn(pool: &DbPool) -> DbConn {
    pool.get().expect("Failed to get DB connection")
}

pub fn initialize() -> DbPool {
    init_logger();
    ensure_server_settings_imported();
    init_pool()
}

/// This struct must match the `server_settings` table in `schema.rs`
#[derive(Queryable, Insertable, AsChangeset, Debug, Clone, Default)]
#[diesel(table_name = server_settings)]
pub struct ServerSettingsRow {
    pub id: i32,
    pub auto_approve_devices: bool,
    pub auto_refresh_enabled: bool,
    pub auto_refresh_seconds: i64,
    pub default_action_ttl_seconds: i64,
    pub action_polling_enabled: bool,
    pub ping_target_ip: String,
    pub force_https: bool,
}

/// Load server settings (returns a DB row struct). If not found, create a default row and return it.
pub fn load_settings(conn: &mut SqliteConnection) -> QueryResult<ServerSettingsRow> {
    use crate::schema::server_settings::dsl as ss_dsl;

    match ss_dsl::server_settings.first::<ServerSettingsRow>(conn) {
        Ok(s) => Ok(s),
        Err(diesel::result::Error::NotFound) => {
            let default = ServerSettingsRow {
                id: 1,
                auto_approve_devices: false,
                auto_refresh_enabled: true,
                auto_refresh_seconds: 30,
                default_action_ttl_seconds: 3600,
                action_polling_enabled: true,
                ping_target_ip: "8.8.8.8".to_string(),
                force_https: false,
            };
            diesel::insert_into(ss_dsl::server_settings)
                .values(&default)
                .execute(conn)?;
            Ok(default)
        }
        Err(e) => Err(e),
    }
}

/// Save server settings row (replace the single-row record)
pub fn save_settings(conn: &mut SqliteConnection, settings: &ServerSettingsRow) -> QueryResult<()> {
    use crate::schema::server_settings::dsl as ss_dsl;
    // Use replace_into to overwrite the single row
    diesel::replace_into(ss_dsl::server_settings)
        .values(settings)
        .execute(conn)?;
    Ok(())
}

/// New history record (Insertable)
#[derive(Insertable)]
#[diesel(table_name = history_log)]
pub struct NewHistory<'a> {
    pub action_id: i64,
    pub device_name: Option<&'a str>,
    pub actor: Option<&'a str>,
    pub action_type: &'a str,
    pub details: Option<&'a str>,
    pub created_at: NaiveDateTime,
}

/// Insert a history entry (accepts a NewHistory or constructs from a HistoryLog)
pub fn insert_history(conn: &mut SqliteConnection, entry: &NewHistory<'_>) -> QueryResult<usize> {
    use crate::schema::history_log::dsl as hl_dsl;
    diesel::insert_into(hl_dsl::history_log)
        .values(entry)
        .execute(conn)
}

/// New audit record (Insertable) - lifetimes for borrowed str values
#[derive(Insertable)]
#[diesel(table_name = audit)]
pub struct NewAudit<'a> {
    pub actor: &'a str,
    pub action_type: &'a str,
    pub target: Option<&'a str>,
    pub details: Option<&'a str>,
    pub created_at: NaiveDateTime,
}

/// Insert an audit entry from an AuditLog struct by converting into NewAudit
pub fn insert_audit(conn: &mut SqliteConnection, entry: &AuditLog) -> QueryResult<usize> {
    use crate::schema::audit::dsl as audit_dsl;

    // Convert AuditLog (owned types) into NewAudit (borrowed references)
    // This code assumes AuditLog fields are named: actor, action_type, target, details, created_at
    let new = NewAudit {
        actor: &entry.actor,
        action_type: &entry.action_type,
        target: entry.target.as_deref(),
        details: entry.details.as_deref(),
        created_at: entry.created_at,
    };

    diesel::insert_into(audit_dsl::audit)
        .values(&new)
        .execute(conn)
}

/// Synchronous convenience: log audit using a DB connection
/// Returns Diesel QueryResult<()>
pub fn log_audit(
    conn: &mut SqliteConnection,
    username_val: &str,
    action_val: &str,
    target_val: Option<&str>,
    details_val: Option<&str>,
) -> QueryResult<()> {
    use crate::schema::audit::dsl as audit_dsl;

    let new_audit = NewAudit {
        actor: username_val,
        action_type: action_val,
        target: target_val,
        details: details_val,
        created_at: Utc::now().naive_utc(),
    };
    diesel::insert_into(audit_dsl::audit)
        .values(&new_audit)
        .execute(conn)?;
    Ok(())
}

/// Update action TTL by adjusting expires_at (ensuring it does not exceed server default)
/// `settings_row` is the server_settings row so we can enforce default_action_ttl_seconds
pub fn update_action_ttl(
    conn: &mut SqliteConnection,
    action_id_val: i64,
    new_ttl_seconds: i64,
    settings_row: &ServerSettingsRow,
) -> QueryResult<usize> {
    use crate::schema::actions::dsl as actions_dsl;
    let ttl_to_set = std::cmp::min(new_ttl_seconds, settings_row.default_action_ttl_seconds);
    let new_expiry = Utc::now().naive_utc() + chrono::Duration::seconds(ttl_to_set);
    diesel::update(actions_dsl::actions.filter(actions_dsl::id.eq(action_id_val)))
        .set(actions_dsl::expires_at.eq(new_expiry))
        .execute(conn)
}

/// Fetch remaining TTL (seconds) for a given action (returns remaining seconds as i64)
pub fn fetch_action_ttl(conn: &mut SqliteConnection, action_id_val: i64) -> QueryResult<i64> {
    use crate::schema::actions::dsl as actions_dsl;
    let expires_at: NaiveDateTime = actions_dsl::actions
        .filter(actions_dsl::id.eq(action_id_val))
        .select(actions_dsl::expires_at)
        .first(conn)?;
    let now = Utc::now().naive_utc();
    let remaining = (expires_at - now).num_seconds();
    Ok(std::cmp::max(0, remaining))
}
