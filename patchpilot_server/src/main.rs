use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use r2d2::Pool;
use rocket::{get, post, routes, launch, State};
use rocket::serde::json::Json;
use rocket_dyn_templates::{Template, context};
use chrono::Utc;
use log::{info, error};

mod schema;
mod models;

use models::{Device, NewDevice, DeviceInfo};
use diesel::sqlite::SqliteConnection;

// Type alias for SQLite connection pool
type DbPool = Pool<ConnectionManager<SqliteConnection>>;

// Custom error type
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

fn establish_connection(pool: &DbPool) -> Result<PooledConnection<ConnectionManager<SqliteConnection>>, ApiError> {
    pool.get().map_err(|e| ApiError::ValidationError(format!("Failed to get DB connection: {}", e)))
}

// Basic validation example
fn validate_device_info(info: &DeviceInfo) -> Result<(), ApiError> {
    if info.system_info.cpu < 0.0 {
        return Err(ApiError::ValidationError("CPU usage cannot be negative".into()));
    }
    if info.system_info.ram_total <= 0 {
        return Err(ApiError::ValidationError("RAM total must be positive".into()));
    }
    Ok(())
}

// Separate registration function
fn insert_or_update_device(conn: &mut SqliteConnection, device_id: &str, info: &DeviceInfo) -> Result<Device, ApiError> {
    use crate::schema::devices::dsl::*;

    let new_device = NewDevice::from_device_info(device_id, info);

    diesel::insert_into(devices)
        .values(&new_device)
        .on_conflict(device_name)
        .do_update()
        .set(&new_device)
        .execute(conn)?;

    let updated_device = devices
        .filter(device_name.eq(device_id))
        .first::<Device>(conn)?;

    Ok(updated_device.enrich_for_dashboard())
}

#[post("/devices/<device_id>", format = "json", data = "<device_info>")]
async fn register_or_update_device(
    pool: &State<DbPool>,
    device_id: &str,
    device_info: Json<DeviceInfo>,
) -> Result<Json<Device>, String> {
    validate_device_info(&device_info).map_err(|e| e.message())?;

    let mut conn = establish_connection(pool).map_err(|e| e.message())?;
    match insert_or_update_device(&mut conn, device_id, &device_info) {
        Ok(device) => {
            info!("Device {} registered/updated successfully", device_id);
            Ok(Json(device))
        }
        Err(e) => {
            error!("Failed to register/update device {}: {}", device_id, e.message());
            Err(e.message())
        }
    }
}

#[get("/devices")]
async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, String> {
    use crate::schema::devices::dsl::*;
    let mut conn = establish_connection(pool).map_err(|e| e.message())?;

    let results = devices
        .load::<Device>(&mut conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|d| d.enrich_for_dashboard())
        .collect::<Vec<Device>>();

    Ok(Json(results))
}

// Separate data-fetching function for dashboard
fn fetch_all_devices(conn: &mut SqliteConnection) -> Vec<Device> {
    use crate::schema::devices::dsl::*;
    devices
        .load::<Device>(conn)
        .unwrap_or_default()
        .into_iter()
        .map(|d| d.enrich_for_dashboard())
        .collect()
}

#[get("/")]
async fn dashboard(pool: &State<DbPool>) -> Template {
    let mut conn = establish_connection(pool).expect("DB connection failed for dashboard");
    let all_devices = fetch_all_devices(&mut conn);

    Template::render("dashboard", context! {
        devices: all_devices,
        now: Utc::now().naive_utc(),
    })
}

/// --- NEW FUNCTION: Automatically initialize database schema ---
fn initialize_db(conn: &mut SqliteConnection) -> Result<(), diesel::result::Error> {
    diesel::sql_query(r#"
        CREATE TABLE IF NOT EXISTS devices (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device_name TEXT NOT NULL UNIQUE,
            hostname TEXT NOT NULL,
            os_name TEXT NOT NULL,
            architecture TEXT NOT NULL,
            last_checkin TIMESTAMP NOT NULL,
            approved BOOLEAN NOT NULL,
            cpu FLOAT NOT NULL,
            ram_total BIGINT NOT NULL,
            ram_used BIGINT NOT NULL,
            ram_free BIGINT NOT NULL,
            disk_total BIGINT NOT NULL,
            disk_free BIGINT NOT NULL,
            disk_health TEXT NOT NULL,
            network_throughput BIGINT NOT NULL,
            ping_latency FLOAT,
            device_type TEXT NOT NULL,
            device_model TEXT NOT NULL
        )
    "#).execute(conn)?;
    Ok(())
}

#[launch]
fn rocket() -> _ {
    use std::env;

    env_logger::init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = Pool::builder()
        .build(manager)
        .expect("Failed to create DB pool");

    // --- Initialize DB schema automatically ---
    {
        let mut conn = pool.get().expect("Failed to get DB connection for initialization");
        initialize_db(&mut conn).expect("Failed to initialize database schema");
        info!("✅ Database schema initialized or already exists");
    }

    rocket::build()
        .manage(pool)
        .mount("/api", routes![register_or_update_device, get_devices])
        .mount("/", routes![dashboard])
        .attach(Template::fairing())
}

