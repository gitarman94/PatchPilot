// src/db.rs
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::sqlite::SqliteConnection;
use diesel::sql_query;
use flexi_logger::{Logger, FileSpec, Age, Cleanup, Criterion, Naming};
use chrono::{Utc, NaiveDateTime};
use std::env;
use crate::models::AuditLog;
use crate::schema::{audit, server_settings, history_log};

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;
pub type DbConn = PooledConnection<ConnectionManager<SqliteConnection>>;

/// Initialize logging
pub fn init_logger() {
    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(FileSpec::default().directory("logs"))
        .rotate(Criterion::Age(Age::Day), Naming::Numbers, Cleanup::KeepLogFiles(7))
        .start()
        .unwrap();
}

/// Create DB connection pool and ensure DB is initialized
pub fn init_pool() -> DbPool {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "patchpilot.db".to_string());
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = Pool::builder()
        .build(manager)
        .expect("Failed to create DB pool");

    // Ensure DB tables exist and default server_settings row is inserted
    if let Ok(conn) = pool.get() {
        if let Err(e) = initialize_database(&conn) {
            panic!("Failed to initialize database: {:?}", e);
        }
    } else {
        panic!("Failed to get a connection from the pool");
    }

    pool
}

/// Get a connection from the pool
pub fn get_conn(pool: &DbPool) -> DbConn {
    pool.get().expect("Failed to get DB connection")
}

/// Initialize database tables and default rows if missing
/// This creates every table declared in src/schema.rs so first-run will not panic.
fn initialize_database(conn: &SqliteConnection) -> QueryResult<()> {
    // devices
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS devices (
            id INTEGER PRIMARY KEY,
            device_id BIGINT NOT NULL,
            device_name TEXT NOT NULL,
            hostname TEXT NOT NULL,
            os_name TEXT NOT NULL,
            architecture TEXT NOT NULL,
            last_checkin DATETIME NOT NULL,
            approved INTEGER NOT NULL DEFAULT 0,
            cpu_usage REAL NOT NULL DEFAULT 0.0,
            cpu_count INTEGER NOT NULL DEFAULT 1,
            cpu_brand TEXT NOT NULL DEFAULT '',
            ram_total BIGINT NOT NULL DEFAULT 0,
            ram_used BIGINT NOT NULL DEFAULT 0,
            disk_total BIGINT NOT NULL DEFAULT 0,
            disk_free BIGINT NOT NULL DEFAULT 0,
            disk_health TEXT NOT NULL DEFAULT '',
            network_throughput BIGINT NOT NULL DEFAULT 0,
            device_type TEXT NOT NULL DEFAULT '',
            device_model TEXT NOT NULL DEFAULT '',
            uptime TEXT,
            updates_available INTEGER NOT NULL DEFAULT 0,
            network_interfaces TEXT,
            ip_address TEXT
        );
        "#
    ).execute(conn)?;

    // actions
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS actions (
            id INTEGER PRIMARY KEY,
            action_type TEXT NOT NULL,
            parameters TEXT,
            author TEXT,
            created_at DATETIME NOT NULL,
            expires_at DATETIME NOT NULL,
            canceled INTEGER NOT NULL DEFAULT 0
        );
        "#
    ).execute(conn)?;

    // action_targets
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS action_targets (
            id INTEGER PRIMARY KEY,
            action_id INTEGER NOT NULL,
            device_id INTEGER NOT NULL,
            status TEXT NOT NULL,
            last_update DATETIME NOT NULL,
            response TEXT
        );
        "#
    ).execute(conn)?;

    // history_log
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS history_log (
            id INTEGER PRIMARY KEY,
            action_id INTEGER NOT NULL,
            device_name TEXT,
            actor TEXT,
            action_type TEXT NOT NULL,
            details TEXT,
            created_at DATETIME NOT NULL
        );
        "#
    ).execute(conn)?;

    // audit
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS audit (
            id INTEGER PRIMARY KEY,
            actor TEXT NOT NULL,
            action_type TEXT NOT NULL,
            target TEXT,
            details TEXT,
            created_at DATETIME NOT NULL
        );
        "#
    ).execute(conn)?;

    // users
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at DATETIME NOT NULL
        );
        "#
    ).execute(conn)?;

    // roles
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS roles (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );
        "#
    ).execute(conn)?;

    // user_roles
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS user_roles (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            role_id INTEGER NOT NULL
        );
        "#
    ).execute(conn)?;

    // groups
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS groups (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT
        );
        "#
    ).execute(conn)?;

    // user_groups
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS user_groups (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            group_id INTEGER NOT NULL
        );
        "#
    ).execute(conn)?;

    // server_settings
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS server_settings (
            id INTEGER PRIMARY KEY,
            auto_approve_devices INTEGER NOT NULL DEFAULT 0,
            auto_refresh_enabled INTEGER NOT NULL DEFAULT 1,
            auto_refresh_seconds BIGINT NOT NULL DEFAULT 30,
            default_action_ttl_seconds BIGINT NOT NULL DEFAULT 3600,
            action_polling_enabled INTEGER NOT NULL DEFAULT 1,
            ping_target_ip TEXT NOT NULL DEFAULT '8.8.8.8',
            force_https INTEGER NOT NULL DEFAULT 0
        );
        "#
    ).execute(conn)?;

    // Ensure there is exactly one settings row (id = 1)
    sql_query(
        r#"
        INSERT OR IGNORE INTO server_settings
        (id, auto_approve_devices, auto_refresh_enabled, auto_refresh_seconds, default_action_ttl_seconds, action_polling_enabled, ping_target_ip, force_https)
        VALUES (1, 0, 1, 30, 3600, 1, '8.8.8.8', 0);
        "#
    ).execute(conn)?;

    Ok(())
}

/// Initialize DB (logging + pool + tables)
pub fn initialize() -> DbPool {
    init_logger();
    init_pool()
}

/// ServerSettings row representation (keep as in your code)
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

/// Load server settings (create default if missing)
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

/// Save server settings row (replace single-row record)
pub fn save_settings(conn: &mut SqliteConnection, settings: &ServerSettingsRow) -> QueryResult<()> {
    use crate::schema::server_settings::dsl as ss_dsl;
    diesel::replace_into(ss_dsl::server_settings)
        .values(settings)
        .execute(conn)?;
    Ok(())
}

/// New history record
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

pub fn insert_history(conn: &mut SqliteConnection, entry: &NewHistory<'_>) -> QueryResult<usize> {
    use crate::schema::history_log::dsl as hl_dsl;
    diesel::insert_into(hl_dsl::history_log)
        .values(entry)
        .execute(conn)
}

/// New audit record
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
    use crate::schema::audit::dsl as audit_dsl;
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

/// Convenience logging for audit entries
pub fn log_audit(
    conn: &mut SqliteConnection,
    username_val: &str,
    action_val: &str,
    target_val: Option<&str>,
    details_val: Option<&str>,
) -> QueryResult<()> {
    let audit_entry = AuditLog {
        id: 0,
        actor: username_val.to_string(),
        action_type: action_val.to_string(),
        target: target_val.map(|s| s.to_string()),
        details: details_val.map(|s| s.to_string()),
        created_at: Utc::now().naive_utc(),
    };
    let _ = insert_audit(conn, &audit_entry)?;
    Ok(())
}

/// Update action TTL using server default limits
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

/// Fetch remaining TTL for an action
pub fn fetch_action_ttl(conn: &mut SqliteConnection, action_id_val: i64) -> QueryResult<i64> {
    use crate::schema::actions::dsl as actions_dsl;
    let expires_at: chrono::NaiveDateTime = actions_dsl::actions
        .filter(actions_dsl::id.eq(action_id_val))
        .select(actions_dsl::expires_at)
        .first(conn)?;
    let now = Utc::now().naive_utc();
    Ok(std::cmp::max(0, (expires_at - now).num_seconds()))
}
