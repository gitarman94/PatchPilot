use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::sqlite::SqliteConnection;
use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use flexi_logger::{Logger, FileSpec, Age, Cleanup, Criterion, Naming};
use std::env;
use chrono::Utc;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;
pub type DbConn = PooledConnection<ConnectionManager<SqliteConnection>>;

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

pub fn init_pool() -> DbPool {
    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "patchpilot.db".to_string());

    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder()
        .build(manager)
        .expect("Failed to create DB pool")
}

pub fn run_migrations(conn: &mut SqliteConnection) {
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
}

pub fn get_conn(pool: &DbPool) -> DbConn {
    pool.get().expect("Failed to get DB connection")
}

pub fn initialize() -> DbPool {
    init_logger();
    let pool = init_pool();
    let mut conn = get_conn(&pool);
    run_migrations(&mut conn);
    pool
}

/// Create default admin user if DB is empty
pub fn create_default_admin(conn: &mut SqliteConnection) {
    use crate::schema::users::dsl::*;

    let count: i64 = users.count().get_result(conn).unwrap_or(0);
    if count == 0 {
        let hash = bcrypt::hash("pass1234", bcrypt::DEFAULT_COST).unwrap();
        diesel::insert_into(users)
            .values((
                username.eq("admin"),
                password_hash.eq(hash),
            ))
            .execute(conn)
            .expect("Failed to create default admin");

        println!("✅ Default admin created (admin / pass1234)");
    }
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
