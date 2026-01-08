use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::sqlite::SqliteConnection;
use flexi_logger::{Logger, FileSpec, Age, Cleanup, Criterion, Naming};
use chrono::{Utc, NaiveDateTime};
use std::env;

use crate::models::{ServerSettings, HistoryLog, AuditLog};
use crate::schema::{audit, server_settings, history_log, actions};

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;
pub type DbConn = PooledConnection<ConnectionManager<SqliteConnection>>;

/// Ensure server_settings import is used to silence warning
pub fn ensure_server_settings_imported() {
    let _ = server_settings::table;
}

/// Initialize logger
pub fn init_logger() {
    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(FileSpec::default().directory("logs"))
        .rotate(Criterion::Age(Age::Day), Naming::Numbers, Cleanup::KeepLogFiles(7))
        .start()
        .unwrap();
}

/// Initialize DB connection pool
pub fn init_pool() -> DbPool {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "patchpilot.db".to_string());
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder().build(manager).expect("Failed to create DB pool")
}

/// Get a single connection from the pool
pub fn get_conn(pool: &DbPool) -> DbConn {
    pool.get().expect("Failed to get DB connection")
}

/// Initialize logger and pool (no migrations)
pub fn initialize() -> DbPool {
    init_logger();
    ensure_server_settings_imported();
    init_pool()
}

// SERVER SETTINGS
#[derive(Queryable, Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = server_settings)]
pub struct ServerSettingsRow {
    pub id: i32,
    pub force_https: bool,
    pub default_action_ttl_seconds: i64,
    pub default_pending_ttl_seconds: i64,
    pub enable_logging: bool,
    pub default_role: String,
}

/// Load the server settings (or default if missing)
pub fn load_settings(conn: &mut SqliteConnection) -> QueryResult<ServerSettingsRow> {
    use crate::schema::server_settings::dsl::*;
    match server_settings.first::<ServerSettingsRow>(conn) {
        Ok(s) => Ok(s),
        Err(diesel::result::Error::NotFound) => {
            let default = ServerSettingsRow {
                id: 1,
                force_https: true,
                default_action_ttl_seconds: 3600,
                default_pending_ttl_seconds: 600,
                enable_logging: true,
                default_role: "user".to_string(),
            };
            diesel::insert_into(server_settings::table)
                .values(&default)
                .execute(conn)?;
            Ok(default)
        }
        Err(e) => Err(e),
    }
}

/// Save server settings (insert or update)
pub fn save_settings(conn: &mut SqliteConnection, settings: &ServerSettingsRow) -> QueryResult<()> {
    use crate::schema::server_settings::dsl::*;
    let existing = server_settings.first::<ServerSettingsRow>(conn).optional()?;
    if let Some(row) = existing {
        diesel::update(server_settings.filter(id.eq(row.id)))
            .set(settings)
            .execute(conn)?;
    } else {
        diesel::insert_into(server_settings::table)
            .values(settings)
            .execute(conn)?;
    }
    Ok(())
}

// HISTORY LOG
pub fn insert_history(conn: &mut SqliteConnection, entry: &HistoryLog) -> QueryResult<usize> {
    diesel::insert_into(history_log::table)
        .values(entry)
        .execute(conn)
}

pub fn fetch_history(conn: &mut SqliteConnection) -> QueryResult<Vec<HistoryLog>> {
    history_log::table
        .order(history_log::created_at.desc())
        .load(conn)
}

// AUDIT LOG
#[derive(Insertable)]
#[diesel(table_name = audit)]
pub struct NewAudit<'a> {
    pub actor: &'a str,
    pub action_type: &'a str,
    pub target: Option<&'a str>,
    pub details: Option<&'a str>,
    pub created_at: NaiveDateTime,
}

pub fn insert_audit(conn: &mut SqliteConnection, entry: &AuditLog) -> QueryResult<usize> {
    diesel::insert_into(audit::table)
        .values(entry)
        .execute(conn)
}

pub fn fetch_audit(conn: &mut SqliteConnection) -> QueryResult<Vec<AuditLog>> {
    audit::table
        .order(audit::created_at.desc())
        .load(conn)
}

/// Helper: audit logging
pub fn log_audit(
    conn: &mut SqliteConnection,
    username_val: &str,
    action_val: &str,
    target_val: Option<&str>,
    details_val: Option<&str>,
) -> QueryResult<()> {
    let new_audit = NewAudit {
        actor: username_val,
        action_type: action_val,
        target: target_val,
        details: details_val,
        created_at: Utc::now().naive_utc(),
    };
    diesel::insert_into(audit::table)
        .values(&new_audit)
        .execute(conn)?;
    Ok(())
}

// ACTION TTL
pub fn update_action_ttl(
    conn: &mut SqliteConnection,
    action_id_val: i64,
    new_ttl: i64,
    settings: &ServerSettingsRow,
) -> QueryResult<usize> {
    let ttl_to_set = std::cmp::min(new_ttl, settings.default_action_ttl_seconds);
    diesel::update(actions::table.filter(actions::id.eq(action_id_val)))
        .set(actions::default_action_ttl_seconds.eq(ttl_to_set))
        .execute(conn)
}

pub fn fetch_action_ttl(conn: &mut SqliteConnection, action_id_val: i64) -> QueryResult<i64> {
    actions::table
        .filter(actions::id.eq(action_id_val))
        .select(actions::default_action_ttl_seconds)
        .first(conn)
}
