use diesel::prelude::*;
use diesel::sqlite::Sqlite;
use chrono::{NaiveDateTime, Utc};
use rocket::serde::{Serialize, Deserialize};

use crate::schema::devices;

#[derive(Queryable, Selectable, Serialize, Deserialize, Debug)]
#[diesel(table_name = devices)]
#[diesel(check_for_backend(Sqlite))]
pub struct Device {
    pub id: i32,
    pub device_name: String,
    pub hostname: String,
    pub os_name: String,
    pub architecture: String,
    pub last_checkin: NaiveDateTime,
    pub approved: bool,
    pub cpu: f32,
    pub ram_total: i64,
    pub ram_used: i64,
    pub ram_free: i64,
    pub disk_total: i64,
    pub disk_free: i64,
    pub disk_health: String,
    pub network_throughput: i64,
    pub ping_latency: f32,
    pub device_type: String,
    pub device_model: String,

    // Computed fields for the dashboard — NOT in the DB
    #[diesel(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<String>,

    #[diesel(skip)]
    #[serde(default)]
    pub updates_available: bool,
}

#[derive(Insertable)]
#[diesel(table_name = devices)]
pub struct NewDevice<'a> {
    pub device_name: &'a str,
    pub hostname: &'a str,
    pub os_name: &'a str,
    pub architecture: &'a str,
    pub last_checkin: NaiveDateTime,
    pub approved: bool,
    pub cpu: f32,
    pub ram_total: i64,
    pub ram_used: i64,
    pub ram_free: i64,
    pub disk_total: i64,
    pub disk_free: i64,
    pub disk_health: &'a str,
    pub network_throughput: i64,
    pub ping_latency: f32,
    pub device_type: &'a str,
    pub device_model: &'a str,
}

impl Device {
    /// Compute uptime as a human-readable string based on last_checkin
    pub fn compute_uptime(&self) -> String {
        let duration = Utc::now().naive_utc() - self.last_checkin;
        let hours = duration.num_hours();
        let minutes = duration.num_minutes() % 60;
        format!("{}h {}m", hours, minutes)
    }

    /// Update computed fields before sending to the frontend
    pub fn enrich_for_dashboard(mut self) -> Self {
        self.uptime = Some(self.compute_uptime());
        self.updates_available = false; // Placeholder
        self
    }
}
