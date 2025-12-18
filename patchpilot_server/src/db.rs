use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::sqlite::SqliteConnection;
use diesel::prelude::*;
use flexi_logger::{Logger, FileSpec, Age, Cleanup, Criterion, Naming};
use std::env;
use chrono::Utc;

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;
pub type DbConn = PooledConnection<ConnectionManager<SqliteConnection>>;

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
    init_pool()
}

/// Create default admin user if DB is empty
pub fn create_default_admin(conn: &mut SqliteConnection) -> Result<(), diesel::result::Error> {
    use crate::schema::{users, roles, user_roles};

    let count: i64 = users::dsl::users.count().get_result(conn)?;
    if count == 0 {
        let hash = bcrypt::hash("pass1234", bcrypt::DEFAULT_COST).unwrap();

        diesel::insert_into(users::dsl::users)
            .values((users::username.eq("admin"), users::password_hash.eq(hash)))
            .execute(conn)?;

        // Fetch IDs
        let admin_id: i32 = users::dsl::users
            .filter(users::dsl::username.eq("admin"))
            .select(users::dsl::id)
            .first(conn)?;

        let admin_role_id: i32 = roles::dsl::roles
            .filter(roles::dsl::name.eq("Admin"))
            .select(roles::dsl::id)
            .first(conn)?;

        // Assign Admin role
        diesel::insert_into(user_roles::dsl::user_roles)
            .values((user_roles::user_id.eq(admin_id), user_roles::role_id.eq(admin_role_id)))
            .execute(conn)?;

        println!("✅ Default admin created (admin / pass1234)");
    }
    Ok(())
}

/// Audit logging helper
pub fn log_audit(
    conn: &mut SqliteConnection,
    actor: &str,
    action: &str,
    target: Option<&str>,
    details: Option<&str>,
) {
    use crate::schema::audit_log::dsl::*;

    diesel::insert_into(audit_log)
        .values((
            actor.eq(actor),
            action_type.eq(action),
            target.eq(target),
            details.eq(details),
            created_at.eq(Utc::now().naive_utc()),
        ))
        .execute(conn)
        .expect("Failed to write audit log");
}
