use diesel::prelude::*;
use chrono::{NaiveDateTime, Utc};
use rocket::serde::{Serialize, Deserialize};

use crate::schema::{
    devices,
    actions,
    action_targets,
    history_log,
    audit,
};

// Devices

#[derive(Queryable, Identifiable, Selectable, Serialize, Deserialize, Debug)]
#[diesel(table_name = devices)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Device {
    pub id: i32,
    pub device_id: String,
    pub device_name: String,
    pub hostname: String,
    pub os_name: String,
    pub architecture: String,
    pub last_checkin: NaiveDateTime,
    pub approved: bool,

    pub cpu_usage: f32,
    pub cpu_count: i32,
    pub cpu_brand: String,

    pub ram_total: i64,
    pub ram_used: i64,

    pub disk_total: i64,
    pub disk_free: i64,
    pub disk_health: String,

    pub network_throughput: i64,

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
    pub device_id: String,
    pub device_name: String,
    pub hostname: String,
    pub os_name: String,
    pub architecture: String,
    pub last_checkin: NaiveDateTime,
    pub approved: bool,

    pub cpu_usage: f32,
    pub cpu_count: i32,
    pub cpu_brand: String,

    pub ram_total: i64,
    pub ram_used: i64,

    pub disk_total: i64,
    pub disk_free: i64,
    pub disk_health: String,

    pub network_throughput: i64,

    pub device_type: String,
    pub device_model: String,
    pub uptime: Option<String>,
    pub updates_available: bool,

    pub network_interfaces: Option<String>,
    pub ip_address: Option<String>,
}

// System Payloads

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemInfo {
    pub os_name: String,
    pub architecture: String,

    pub cpu_usage: f32,
    pub cpu_count: i32,
    pub cpu_brand: String,

    pub ram_total: i64,
    pub ram_used: i64,

    pub disk_total: i64,
    pub disk_free: i64,
    pub disk_health: String,

    pub network_throughput: i64,

    pub network_interfaces: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub system_info: SystemInfo,
    pub device_type: Option<String>,
    pub device_model: Option<String>,
}

// Actions

#[derive(Debug, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = actions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Action {
    pub id: String,
    pub action_type: String,
    pub parameters: Option<String>,
    pub author: Option<String>,
    pub created_at: NaiveDateTime,
    pub expires_at: NaiveDateTime,
    pub canceled: bool,
}

#[derive(Debug, Insertable, Serialize, Deserialize)]
#[diesel(table_name = actions)]
pub struct NewAction {
    pub id: String,
    pub action_type: String,
    pub parameters: Option<String>,
    pub author: Option<String>,
    pub created_at: NaiveDateTime,
    pub expires_at: NaiveDateTime,
    pub canceled: bool,
}

#[derive(Debug, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = action_targets)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ActionTarget {
    pub id: i32,
    pub action_id: String,
    pub device_id: String,
    pub status: String,
    pub last_update: NaiveDateTime,
    pub response: Option<String>,
}

// History

#[derive(Debug, Queryable, Selectable, Serialize)]
#[diesel(table_name = history_log)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct HistoryLog {
    pub id: i32,
    pub action_id: Option<String>,
    pub device_name: Option<String>,
    pub actor: Option<String>,
    pub action_type: String,
    pub details: Option<String>,
    pub created_at: NaiveDateTime,
}

// Audit

#[derive(Debug, Queryable, Insertable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = audit)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct AuditLog {
    pub id: i32,
    pub actor: String,
    pub action_type: String,
    pub target: Option<String>,
    pub details: Option<String>,
    pub created_at: NaiveDateTime,
}

// Device Logic

impl DeviceInfo {
    pub fn merge_with(&mut self, other: &DeviceInfo) {
        let s = &mut self.system_info;
        let o = &other.system_info;

        if !o.os_name.is_empty()      { s.os_name = o.os_name.clone(); }
        if !o.architecture.is_empty() { s.architecture = o.architecture.clone(); }
        if !o.cpu_brand.is_empty()    { s.cpu_brand = o.cpu_brand.clone(); }
        if !o.disk_health.is_empty()  { s.disk_health = o.disk_health.clone(); }

        if let Some(ip) = &o.ip_address {
            if !ip.is_empty() { s.ip_address = Some(ip.clone()); }
        }

        if let Some(nics) = &o.network_interfaces {
            if !nics.is_empty() { s.network_interfaces = Some(nics.clone()); }
        }

        s.cpu_usage = o.cpu_usage;
        s.cpu_count = o.cpu_count;
        s.ram_total = o.ram_total;
        s.ram_used  = o.ram_used;
        s.disk_total = o.disk_total;
        s.disk_free  = o.disk_free;
        s.network_throughput = o.network_throughput;

        if let Some(t) = &other.device_type {
            if !t.is_empty() { self.device_type = Some(t.clone()); }
        }

        if let Some(m) = &other.device_model {
            if !m.is_empty() { self.device_model = Some(m.clone()); }
        }

        if !other.device_id.is_empty() {
            self.device_id = other.device_id.clone();
        }
    }

    pub fn to_device(&self, device_id: &str) -> Device {
        let s = &self.system_info;

        Device {
            id: 0,
            device_id: device_id.to_string(),
            device_name: device_id.to_string(),
            hostname: device_id.to_string(),

            os_name: s.os_name.clone(),
            architecture: s.architecture.clone(),
            last_checkin: Utc::now().naive_utc(),
            approved: false,

            cpu_usage: s.cpu_usage,
            cpu_count: s.cpu_count,
            cpu_brand: s.cpu_brand.clone(),

            ram_total: s.ram_total,
            ram_used: s.ram_used,

            disk_total: s.disk_total,
            disk_free: s.disk_free,
            disk_health: s.disk_health.clone(),

            network_throughput: s.network_throughput,

            device_type: self.device_type.clone().unwrap_or_default(),
            device_model: self.device_model.clone().unwrap_or_default(),

            uptime: Some("0h 0m".into()),
            updates_available: false,

            network_interfaces: s.network_interfaces.clone(),
            ip_address: s.ip_address.clone(),
        }
    }
}

impl NewDevice {
    pub fn from_device_info(device_id: &str, info: &DeviceInfo, existing: Option<&Device>) -> Self {
        let s = &info.system_info;
        NewDevice {
            device_id: device_id.to_string(),
            device_name: device_id.to_string(),
            hostname: device_id.to_string(),
            os_name: s.os_name.clone(),
            architecture: s.architecture.clone(),
            last_checkin: Utc::now().naive_utc(),
            approved: existing.map_or(false, |e| e.approved),

            cpu_usage: s.cpu_usage,
            cpu_count: s.cpu_count,
            cpu_brand: s.cpu_brand.clone(),

            ram_total: s.ram_total,
            ram_used: s.ram_used,

            disk_total: s.disk_total,
            disk_free: s.disk_free,
            disk_health: s.disk_health.clone(),

            network_throughput: s.network_throughput,

            device_type: info.device_type.clone().unwrap_or_default(),
            device_model: info.device_model.clone().unwrap_or_default(),

            uptime: Some("0h 0m".into()),
            updates_available: false,

            network_interfaces: s.network_interfaces.clone(),
            ip_address: s.ip_address.clone(),
        }
    }
}

impl Device {
    pub fn compute_uptime(&self) -> String {
        let duration = Utc::now().naive_utc() - self.last_checkin;
        format!("{}h {}m", duration.num_hours(), duration.num_minutes() % 60)
    }

    pub fn enrich_for_dashboard(mut self) -> Self {
        self.uptime = Some(self.compute_uptime());
        self
    }

    pub fn from_info(device_id: &str, info: &DeviceInfo) -> Self {
        info.to_device(device_id)
    }
}
