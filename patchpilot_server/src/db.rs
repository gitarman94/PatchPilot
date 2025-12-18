use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::sqlite::SqliteConnection;
use diesel::prelude::*;
use flexi_logger::{Logger, FileSpec, Age, Cleanup, Criterion, Naming};
use std::env;
use chrono::Utc;

use crate::schema::audit;

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
    username: &str,
    action: &str,
    target_val: Option<&str>,
    details_val: Option<&str>,
) -> Result<(), diesel::result::Error> {
    use crate::schema::audit::dsl::*;

    let new_audit = NewAudit {
        username,
        action,
        target: target_val,
        details: details_val,
        created_at: Utc::now().naive_utc(),
    };

    diesel::insert_into(audit)
        .values(&new_audit)
        .execute(conn)?;

    Ok(())
}

// Define struct for audit
#[derive(Insertable)]
#[diesel(table_name = audit)]
pub struct NewAudit<'a> {
    pub actor: &'a str,
    pub action_type: &'a str,
    pub target: Option<&'a str>,
    pub details: Option<&'a str>,
}

