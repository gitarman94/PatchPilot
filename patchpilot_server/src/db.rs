use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::sqlite::SqliteConnection;
use diesel::prelude::*;
use flexi_logger::{Logger, FileSpec, Age, Cleanup, Criterion, Naming};
use std::env;
use chrono::Utc;

use crate::models::{ServerSettings, DeviceInfo};
use crate::schema::{audit, server_settings, users, roles, user_roles};

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
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Numbers,
            Cleanup::KeepLogFiles(7),
        )
        .start()
        .unwrap();
}

/// Initialize DB connection pool
pub fn init_pool() -> DbPool {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "patchpilot.db".to_string());
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder()
        .build(manager)
        .expect("Failed to create DB pool")
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

/// Create default admin user if DB is empty
pub fn create_default_admin(conn: &mut SqliteConnection) -> Result<(), diesel::result::Error> {
    use crate::schema::users::dsl::{users, username, password_hash, id as user_id};
    use crate::schema::roles::dsl::{roles, name, id as role_id};
    use crate::schema::user_roles::dsl::{user_roles, user_id as ur_user_id, role_id as ur_role_id};

    let count: i64 = users.count().get_result(conn)?;
    if count == 0 {
        let hash = bcrypt::hash("pass1234", bcrypt::DEFAULT_COST).unwrap();

        diesel::insert_into(users)
            .values((username.eq("admin"), password_hash.eq(hash)))
            .execute(conn)?;

        // Fetch IDs
        let admin_id_val: i32 = users
            .filter(username.eq("admin"))
            .select(user_id)
            .first(conn)?;

        let admin_role_id_val: i32 = roles
            .filter(name.eq("Admin"))
            .select(role_id)
            .first(conn)?;

        // Assign Admin role
        diesel::insert_into(user_roles)
            .values((ur_user_id.eq(admin_id_val), ur_role_id.eq(admin_role_id_val)))
            .execute(conn)?;

        println!("✅ Default admin created (admin / pass1234)");
    }
    Ok(())
}

/// Audit logging helper
pub fn log_audit(
    conn: &mut SqliteConnection,
    username_val: &str,
    action_val: &str,
    target_val: Option<&str>,
    details_val: Option<&str>,
) -> Result<(), diesel::result::Error> {
    use crate::schema::audit::dsl::*;

    let new_audit = NewAudit {
        actor: username_val,
        action_type: action_val,
        target: target_val,
        details: details_val,
        created_at: Utc::now().naive_utc(),
    };

    diesel::insert_into(audit)
        .values(&new_audit)
        .execute(conn)?;

    Ok(())
}

/// Struct for audit entries
#[derive(Insertable)]
#[diesel(table_name = audit)]
pub struct NewAudit<'a> {
    pub actor: &'a str,
    pub action_type: &'a str,
    pub target: Option<&'a str>,
    pub details: Option<&'a str>,
    pub created_at: chrono::NaiveDateTime,
}

/// Get current server settings from DB
pub fn load_settings(conn: &mut SqliteConnection) -> Result<ServerSettings, diesel::result::Error> {
    use crate::schema::server_settings::dsl::*;

    let row = server_settings.first::<ServerSettingsRow>(conn).optional()?;

    Ok(match row {
        Some(s) => ServerSettings {
            id: s.id as i64,
            force_https: s.force_https,
            max_action_ttl: s.max_action_ttl,
            max_pending_age: s.max_pending_age,
            enable_logging: s.enable_logging,
            default_role: s.default_role,
        },
        None => ServerSettings {
            id: 1,
            force_https: false,
            max_action_ttl: 3600,
            max_pending_age: 86400,
            enable_logging: true,
            default_role: "User".to_string(),
        },
    })
}

/// Save server settings to DB (insert or update)
pub fn save_settings(conn: &mut SqliteConnection, settings: &ServerSettings) -> Result<(), diesel::result::Error> {
    use crate::schema::server_settings::dsl::*;

    let existing = server_settings.first::<ServerSettingsRow>(conn).optional()?;

    if let Some(row) = existing {
        diesel::update(server_settings.filter(id.eq(row.id)))
            .set((
                force_https.eq(settings.force_https),
                max_action_ttl.eq(settings.max_action_ttl),
                max_pending_age.eq(settings.max_pending_age),
                enable_logging.eq(settings.enable_logging),
                default_role.eq(&settings.default_role),
            ))
            .execute(conn)?;
    } else {
        diesel::insert_into(server_settings)
            .values((
                force_https.eq(settings.force_https),
                max_action_ttl.eq(settings.max_action_ttl),
                max_pending_age.eq(settings.max_pending_age),
                enable_logging.eq(settings.enable_logging),
                default_role.eq(&settings.default_role),
            ))
            .execute(conn)?;
    }

    Ok(())
}

/// Struct representing a row in server_settings
#[derive(Queryable)]
pub struct ServerSettingsRow {
    pub id: i32,
    pub force_https: bool,
    pub max_action_ttl: i64,
    pub max_pending_age: i64,
    pub enable_logging: bool,
    pub default_role: String,
}
