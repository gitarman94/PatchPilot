use diesel::prelude::*;
use chrono::{NaiveDateTime, Utc};
use rocket::serde::{Serialize, Deserialize};

use crate::schema::devices;

#[derive(Queryable, Serialize, Deserialize, Debug)]
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

    #[diesel(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<String>,

    #[diesel(skip)]
    #[serde(default)]
    pub updates_available: bool,
}

#[derive(Insertable)]
#[diesel(table_name = devices)]
pub struct NewDevice {
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
}

impl Device {
    pub fn compute_uptime(&self) -> String {
        let duration = Utc::now().naive_utc() - self.last_checkin;
        let hours = duration.num_hours();
        let minutes = duration.num_minutes() % 60;
        format!("{}h {}m", hours, minutes)
    }

    pub fn enrich_for_dashboard(mut self) -> Self {
        self.uptime = Some(self.compute_uptime());
        self.updates_available = false; // Placeholder
        self
    }
}

impl NewDevice {
    pub fn from_device_info(device_id: &str, info: &crate::main::DeviceInfo) -> Self {
        Self {
            device_name: device_id.to_string(),
            hostname: device_id.to_string(),
            os_name: info.system_info.os_name.clone(),
            architecture: info.system_info.architecture.clone(),
            last_checkin: Utc::now().naive_utc(),
            approved: false,
            cpu: info.system_info.cpu,
            ram_total: info.system_info.ram_total,
            ram_used: info.system_info.ram_used,
            ram_free: info.system_info.ram_free,
            disk_total: info.system_info.disk_total,
            disk_free: info.system_info.disk_free,
            disk_health: info.system_info.disk_health.clone(),
            network_throughput: info.system_info.network_throughput,
            ping_latency: info.system_info.ping_latency.unwrap_or(0.0),
            device_type: info.device_type.clone().unwrap_or_default(),
            device_model: info.device_model.clone().unwrap_or_default(),
        }
    }
}
