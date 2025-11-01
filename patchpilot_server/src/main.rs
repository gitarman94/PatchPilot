use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use r2d2::Pool;
use rocket::{get, post, routes, launch, State};
use rocket::serde::{json::Json, Deserialize};
use rocket_dyn_templates::{Template, context};
use chrono::Utc;

mod schema;
mod models;

use models::{Device, NewDevice};
use diesel::sqlite::SqliteConnection;

// Type alias for SQLite connection pool
type DbPool = Pool<ConnectionManager<SqliteConnection>>;

// Helper to get a DB connection
fn establish_connection(pool: &DbPool) -> PooledConnection<ConnectionManager<SqliteConnection>> {
    pool.get().expect("Failed to get a DB connection from the pool.")
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct DeviceInfo {
    pub device_type: Option<String>,
    pub device_model: Option<String>,
    pub system_info: SystemInfo,
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct SystemInfo {
    pub os_name: String,
    pub architecture: String,
    pub cpu: f32,
    pub ram_total: i64,
    pub ram_used: i64,
    pub ram_free: i64,
    pub disk_total: i64,
    pub disk_free: i64,
    pub disk_health: String,
    pub network_throughput: i64,
    pub ping_latency: Option<f32>,
}

#[post("/devices/<device_id>", format = "json", data = "<device_info>")]
async fn register_or_update_device(
    pool: &State<DbPool>,
    device_id: &str,
    device_info: Json<DeviceInfo>,
) -> Result<Json<Device>, String> {
    use crate::schema::devices::dsl::*;
    let mut conn = establish_connection(pool);

    // Convert DeviceInfo into NewDevice (handles cloning and Option safely)
    let new_device = NewDevice::from_device_info(device_id, &device_info);

    diesel::insert_into(devices)
        .values(&new_device)
        .on_conflict(device_name)
        .do_update()
        .set((
            cpu.eq(new_device.cpu),
            ram_total.eq(new_device.ram_total),
            ram_used.eq(new_device.ram_used),
            ram_free.eq(new_device.ram_free),
            disk_total.eq(new_device.disk_total),
            disk_free.eq(new_device.disk_free),
            network_throughput.eq(new_device.network_throughput),
            ping_latency.eq(new_device.ping_latency),
            last_checkin.eq(new_device.last_checkin),
        ))
        .execute(&mut conn)
        .map_err(|e| e.to_string())?;

    let result = devices
        .filter(device_name.eq(device_id))
        .first::<Device>(&mut conn)
        .map_err(|e| e.to_string())?
        .enrich_for_dashboard();

    Ok(Json(result))
}

#[get("/devices")]
async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, String> {
    use crate::schema::devices::dsl::*;
    let mut conn = establish_connection(pool);

    let results = devices
        .load::<Device>(&mut conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|d| d.enrich_for_dashboard())
        .collect::<Vec<Device>>();

    Ok(Json(results))
}

#[get("/")]
async fn dashboard(pool: &State<DbPool>) -> Template {
    use crate::schema::devices::dsl::*;
    let mut conn = establish_connection(pool);

    let all_devices = devices
        .load::<Device>(&mut conn)
        .unwrap_or_default()
        .into_iter()
        .map(|d| d.enrich_for_dashboard())
        .collect::<Vec<Device>>();

    Template::render("dashboard", context! {
        devices: all_devices,
        now: Utc::now().naive_utc(),
    })
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

    rocket::build()
        .manage(pool)
        .mount("/api", routes![register_or_update_device, get_devices])
        .mount("/", routes![dashboard])
        .attach(Template::fairing())
}
