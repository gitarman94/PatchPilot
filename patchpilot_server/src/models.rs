use diesel::prelude::*;
use chrono::{NaiveDateTime, Utc};
use rocket::serde::{Serialize, Deserialize};

use crate::schema::devices;

#[derive(Queryable, Serialize, Deserialize, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
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
    pub ping_latency: Option<f32>,
    pub device_type: String,
    pub device_model: String,
    pub uptime: Option<String>,
    pub updates_available: bool,
    pub network_interfaces: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Insertable, AsChangeset)]
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
    pub ping_latency: Option<f32>,
    pub device_type: String,
    pub device_model: String,
    pub uptime: Option<String>,
    pub updates_available: bool,
    pub network_interfaces: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    pub network_interfaces: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub system_info: SystemInfo,
    pub device_type: Option<String>, 
    pub device_model: Option<String>,
}

impl DeviceInfo {
    /// Helper: convert DeviceInfo into a Device struct
    pub fn to_device(&self, device_id: &str) -> Device {
        Device {
            id: 0,
            device_name: device_id.to_string(),
            hostname: device_id.to_string(),
            os_name: self.system_info.os_name.clone(),
            architecture: self.system_info.architecture.clone(),
            last_checkin: Utc::now().naive_utc(),
            approved: false,
            cpu: self.system_info.cpu,
            ram_total: self.system_info.ram_total,
            ram_used: self.system_info.ram_used,
            ram_free: self.system_info.ram_free,
            disk_total: self.system_info.disk_total,
            disk_free: self.system_info.disk_free,
            disk_health: self.system_info.disk_health.clone(),
            network_throughput: self.system_info.network_throughput,
            ping_latency: self.system_info.ping_latency,
            device_type: self.device_type.clone().unwrap_or_default(),
            device_model: self.device_model.clone().unwrap_or_default(),
            uptime: Some("0h 0m".to_string()),
            updates_available: false,
            network_interfaces: self.system_info.network_interfaces.clone(),
            ip_address: self.system_info.ip_address.clone(),
        }
    }
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
        self
    }

    pub fn from_info(device_id: &str, info: &DeviceInfo) -> Self {
        info.to_device(device_id)
    }
}

impl NewDevice {
    pub fn from_device_info(device_id: &str, info: &DeviceInfo) -> Self {
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
            ping_latency: info.system_info.ping_latency,
            device_type: info.device_type.clone().unwrap_or_default(),
            device_model: info.device_model.clone().unwrap_or_default(),
            uptime: Some("0h 0m".to_string()),
            updates_available: false,
            network_interfaces: info.system_info.network_interfaces.clone(),
            ip_address: info.system_info.ip_address.clone(),
        }
    }
}
