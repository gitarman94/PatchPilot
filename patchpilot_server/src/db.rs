use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::sqlite::SqliteConnection;
use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use flexi_logger::{Logger, FileSpec, Age, Cleanup, Criterion, Naming};
use std::env;
use chrono::Utc;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

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

/// Run migrations
pub fn run_migrations(conn: &mut SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
    conn.run_pending_migrations(MIGRATIONS)?;
    Ok(())
}

/// Get pooled connection
pub fn get_conn(pool: &DbPool) -> DbConn {
    pool.get().expect("Failed to get DB connection from pool")
}

/// Initialize DB + logger + migrations
pub fn initialize() -> DbPool {
    init_logger();
    let pool = init_pool();
    let mut conn = get_conn(&pool);
    run_migrations(&mut conn).expect("Failed to run migrations");
    pool
}

/// Create default admin user if no users exist
pub fn create_default_admin(conn: &mut SqliteConnection) -> Result<(), diesel::result::Error> {
    use crate::schema::users::dsl::*;
    use diesel::dsl::*;

    let count: i64 = users.count().get_result(conn)?;
    if count == 0 {
        let default_pass_hash = bcrypt::hash("pass1234", bcrypt::DEFAULT_COST).unwrap();
        diesel::insert_into(users)
            .values((
                username.eq("admin"),
                password_hash.eq(default_pass_hash),
            ))
            .execute(conn)?;
        println!("Created default admin user with username 'admin' and password 'pass1234'");
    }
    Ok(())
}

/// Log audit events
pub fn log_audit(
    conn: &mut SqliteConnection,
    actor: &str,
    action_type: &str,
    target: Option<&str>,
    details: Option<&str>,
) {
    use crate::schema::audit_log;
    diesel::insert_into(audit_log::table)
        .values((
            audit_log::actor.eq(actor),
            audit_log::action_type.eq(action_type),
            audit_log::target.eq(target),
            audit_log::details.eq(details),
            audit_log::created_at.eq(Utc::now().naive_utc())
        ))
        .execute(conn)
        .unwrap();
}
