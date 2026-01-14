use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::sqlite::SqliteConnection;
use diesel::sql_query;
use flexi_logger::{Logger, FileSpec, Age, Cleanup, Criterion, Naming};
use chrono::{Utc, NaiveDateTime};
use std::env;
use std::fs::OpenOptions;
use std::path::Path;

use crate::schema::*;
use crate::models::AuditLog;

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;
pub type DbConn = PooledConnection<ConnectionManager<SqliteConnection>>;

/// Initialize logger and DB pool
pub fn initialize() -> DbPool {
    init_logger();
    init_pool()
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

fn init_pool() -> DbPool {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "/opt/patchpilot_server/patchpilot.db".to_string());

    // Ensure DB file exists (create parent dirs if needed)
    if !Path::new(&database_url).exists() {
        if let Some(parent) = Path::new(&database_url).parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).expect("Failed to create DB parent directories");
            }
        }
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(&database_url)
            .expect("Failed to create database file");
    }

    let manager = ConnectionManager::<SqliteConnection>::new(database_url.clone());
    let pool = Pool::builder()
        .build(manager)
        .expect("Failed to create DB pool");

    {
        let mut conn = pool.get().expect("Failed to get DB connection");
        initialize_database(&mut conn).expect("DB initialization failed");
    }

    pool
}

pub fn get_conn(pool: &DbPool) -> DbConn {
    pool.get().expect("Failed to get DB connection")
}

/// Initialize database tables
fn initialize_database(conn: &mut SqliteConnection) -> QueryResult<()> {
    // devices table
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
            approved BOOLEAN NOT NULL DEFAULT 0,
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
            uptime BIGINT,
            updates_available BOOLEAN NOT NULL DEFAULT 0,
            network_interfaces TEXT,
            ip_address TEXT
        );
        "#
    )
    .execute(conn)?;

    // actions table
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS actions (
            id INTEGER PRIMARY KEY,
            action_type TEXT NOT NULL,
            parameters TEXT,
            author TEXT,
            created_at DATETIME NOT NULL,
            expires_at DATETIME NOT NULL,
            canceled BOOLEAN NOT NULL DEFAULT 0
        );
        "#
    )
    .execute(conn)?;

    // action_targets table
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS action_targets (
            id INTEGER PRIMARY KEY,
            action_id BIGINT NOT NULL,
            device_id BIGINT NOT NULL,
            status TEXT NOT NULL,
            last_update DATETIME NOT NULL,
            response TEXT
        );
        "#
    )
    .execute(conn)?;

    // history_log table
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS history_log (
            id INTEGER PRIMARY KEY,
            action_id BIGINT NOT NULL,
            device_name TEXT,
            actor TEXT,
            action_type TEXT NOT NULL,
            details TEXT,
            created_at DATETIME NOT NULL
        );
        "#
    )
    .execute(conn)?;

    // audit table
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
    )
    .execute(conn)?;

    // users table
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at DATETIME NOT NULL
        );
        "#
    )
    .execute(conn)?;

    // roles table
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS roles (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );
        "#
    )
    .execute(conn)?;

    // user_roles table
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS user_roles (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            role_id INTEGER NOT NULL
        );
        "#
    )
    .execute(conn)?;

    // groups table
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS groups (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT
        );
        "#
    )
    .execute(conn)?;

    // user_groups table
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS user_groups (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            group_id INTEGER NOT NULL
        );
        "#
    )
    .execute(conn)?;

    // server_settings table
    sql_query(
        r#"
        CREATE TABLE IF NOT EXISTS server_settings (
            id INTEGER PRIMARY KEY,
            auto_approve_devices BOOLEAN NOT NULL DEFAULT 0,
            auto_refresh_enabled BOOLEAN NOT NULL DEFAULT 1,
            auto_refresh_seconds BIGINT NOT NULL DEFAULT 30,
            default_action_ttl_seconds BIGINT NOT NULL DEFAULT 3600,
            action_polling_enabled BOOLEAN NOT NULL DEFAULT 1,
            ping_target_ip TEXT NOT NULL DEFAULT '8.8.8.8',
            force_https BOOLEAN NOT NULL DEFAULT 0
        );
        "#
    )
    .execute(conn)?;

    // insert default server_settings row
    sql_query(
        r#"
        INSERT OR IGNORE INTO server_settings
        (id, auto_approve_devices, auto_refresh_enabled, auto_refresh_seconds,
         default_action_ttl_seconds, action_polling_enabled, ping_target_ip, force_https)
        VALUES (1, 0, 1, 30, 3600, 1, '8.8.8.8', 0);
        "#
    )
    .execute(conn)?;

    Ok(())
}


// DATABASE STRUCTS
// SERVER SETTINGS STRUCT
#[derive(Queryable, Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = server_settings)]
pub struct ServerSettingsRow { // <-- made public
    pub id: i32,
    pub auto_approve_devices: bool,
    pub auto_refresh_enabled: bool,
    pub auto_refresh_seconds: i64,
    pub default_action_ttl_seconds: i64,
    pub action_polling_enabled: bool,
    pub ping_target_ip: String,
    pub force_https: bool,
}


// LOAD / SAVE SETTINGS
pub fn load_settings(conn: &mut SqliteConnection) -> QueryResult<ServerSettingsRow> {
    server_settings::table.first(conn)
}

pub fn save_settings(
    conn: &mut SqliteConnection,
    settings: &ServerSettingsRow,
) -> QueryResult<()> {
    diesel::replace_into(server_settings::table)
        .values(settings)
        .execute(conn)?;
    Ok(())
}


// HISTORY LOG
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

pub fn insert_history(
    conn: &mut SqliteConnection,
    entry: &NewHistory<'_>,
) -> QueryResult<usize> {
    diesel::insert_into(history_log::table)
        .values(entry)
        .execute(conn)
}


// AUDIT LOG
#[derive(Insertable)]
#[diesel(table_name = audit)]
struct NewAudit<'a> {
    actor: &'a str,
    action_type: &'a str,
    target: Option<&'a str>,
    details: Option<&'a str>,
    created_at: NaiveDateTime,
}

pub fn insert_audit(
    conn: &mut SqliteConnection,
    entry: &AuditLog,
) -> QueryResult<usize> {
    let record = NewAudit {
        actor: &entry.actor,
        action_type: &entry.action_type,
        target: entry.target.as_deref(),
        details: entry.details.as_deref(),
        created_at: entry.created_at,
    };

    diesel::insert_into(audit::table)
        .values(&record)
        .execute(conn)
}

// LOG AUDIT WRAPPER FOR ROUTES
pub fn log_audit(
    conn: &mut SqliteConnection,
    actor: &str,
    action: &str,
    target: Option<&str>,
    details: Option<&str>,
) -> QueryResult<()> {
    let entry = AuditLog {
        id: 0,
        actor: actor.to_string(),
        action_type: action.to_string(),
        target: target.map(str::to_string),
        details: details.map(str::to_string),
        created_at: Utc::now().naive_utc(),
    };

    insert_audit(conn, &entry)?;
    Ok(())
}

pub use log_audit as db_log_audit;


// ACTION TTL HELPERS
pub fn update_action_ttl(
    conn: &mut SqliteConnection,
    action_id_val: i64,
    new_ttl_seconds: i64,
    settings: &ServerSettingsRow,
) -> QueryResult<usize> {
    let ttl = std::cmp::min(new_ttl_seconds, settings.default_action_ttl_seconds);
    let expires = Utc::now().naive_utc() + chrono::Duration::seconds(ttl);

    diesel::update(actions::table.filter(actions::id.eq(action_id_val)))
        .set(actions::expires_at.eq(expires))
        .execute(conn)
}

pub fn fetch_action_ttl(
    conn: &mut SqliteConnection,
    action_id_val: i64,
) -> QueryResult<i64> {
    let expiry: NaiveDateTime = actions::table
        .filter(actions::id.eq(action_id_val))
        .select(actions::expires_at)
        .first(conn)?;

    let now = Utc::now().naive_utc();
    Ok(std::cmp::max(0, (expiry - now).num_seconds()))
}
