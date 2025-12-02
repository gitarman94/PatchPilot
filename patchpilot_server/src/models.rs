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

    // updated to match client
    pub cpu_usage: f32,
    pub cpu_count: i32,
    pub cpu_brand: String,

    pub ram_total: i64,
    pub ram_used: i64,

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

    pub cpu_usage: f32,
    pub cpu_count: i32,
    pub cpu_brand: String,

    pub ram_total: i64,
    pub ram_used: i64,

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

    // updated to match client
    pub cpu_usage: f32,
    pub cpu_count: i32,
    pub cpu_brand: String,

    pub ram_total: i64,
    pub ram_used: i64,

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
    pub fn merge_with(&mut self, other: &DeviceInfo) {
        let s = &mut self.system_info;
        let o = &other.system_info;

        if !o.os_name.is_empty()            { s.os_name = o.os_name.clone(); }
        if !o.architecture.is_empty()       { s.architecture = o.architecture.clone(); }
        if !o.cpu_brand.is_empty()          { s.cpu_brand = o.cpu_brand.clone(); }
        if !o.disk_health.is_empty()        { s.disk_health = o.disk_health.clone(); }

        if let Some(ip) = &o.ip_address {
            if !ip.is_empty() {
                s.ip_address = Some(ip.clone());
            }
        }

        if let Some(nics) = &o.network_interfaces {
            if !nics.is_empty() {
                s.network_interfaces = Some(nics.clone());
            }
        }

        s.cpu_usage = o.cpu_usage;
        s.cpu_count = o.cpu_count;

        s.ram_total = o.ram_total;
        s.ram_used  = o.ram_used;

        s.disk_total = o.disk_total;
        s.disk_free  = o.disk_free;

        s.network_throughput = o.network_throughput;
        s.ping_latency = o.ping_latency;

        if let Some(t) = &other.device_type {
            if !t.is_empty() {
                self.device_type = Some(t.clone());
            }
        }

        if let Some(m) = &other.device_model {
            if !m.is_empty() {
                self.device_model = Some(m.clone());
            }
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
    pub fn from_device_info(device_id: &str, info: &DeviceInfo, existing: Option<&Device>) -> Self {
        // Helpers to merge values safely
        fn pick_string(new: String, old: &str) -> String {
            if new.trim().is_empty() { old.to_string() } else { new }
        }

        fn pick_i64(new: i64, old: i64) -> i64 {
            if new == 0 { old } else { new }
        }

        fn pick_f32(new: f32, old: f32) -> f32 {
            if new == 0.0 { old } else { new }
        }

        fn pick_option<T: Clone>(new: Option<T>, old: Option<T>) -> Option<T> {
            if new.is_none() { old } else { new }
        }

        let si = &info.system_info;

        let (old, approved) = match existing {
            Some(dev) => (dev, dev.approved),
            None => (
                &Device {
                    id: 0,
                    device_name: device_id.to_string(),
                    hostname: device_id.to_string(),
                    os_name: "".into(),
                    architecture: "".into(),
                    last_checkin: Utc::now().naive_utc(),
                    approved: false,
                    cpu_usage: 0.0,
                    cpu_count: 0,
                    cpu_brand: "".into(),
                    ram_total: 0,
                    ram_used: 0,
                    disk_total: 0,
                    disk_free: 0,
                    disk_health: "".into(),
                    network_throughput: 0,
                    ping_latency: None,
                    device_type: "".into(),
                    device_model: "".into(),
                    uptime: Some("0h 0m".into()),
                    updates_available: false,
                    network_interfaces: None,
                    ip_address: None,
                },
                false,
            ),
        };

        Self {
            device_name: device_id.to_string(),
            hostname: device_id.to_string(),

            os_name: pick_string(si.os_name.clone(), &old.os_name),
            architecture: pick_string(si.architecture.clone(), &old.architecture),

            last_checkin: Utc::now().naive_utc(),
            approved,

            cpu_usage: pick_f32(si.cpu_usage, old.cpu_usage),
            cpu_count: if si.cpu_count == 0 { old.cpu_count } else { si.cpu_count },
            cpu_brand: pick_string(si.cpu_brand.clone(), &old.cpu_brand),

            ram_total: pick_i64(si.ram_total, old.ram_total),
            ram_used: pick_i64(si.ram_used, old.ram_used),

            disk_total: pick_i64(si.disk_total, old.disk_total),
            disk_free: pick_i64(si.disk_free, old.disk_free),
            disk_health: pick_string(si.disk_health.clone(), &old.disk_health),

            network_throughput: pick_i64(si.network_throughput, old.network_throughput),
            ping_latency: pick_option(si.ping_latency, old.ping_latency),

            device_type: pick_string(info.device_type.clone().unwrap_or_default(), &old.device_type),
            device_model: pick_string(info.device_model.clone().unwrap_or_default(), &old.device_model),

            uptime: pick_option(Some("0h 0m".into()), old.uptime.clone()),

            updates_available: old.updates_available, // server controls this, not client

            network_interfaces: pick_option(si.network_interfaces.clone(), old.network_interfaces.clone()),
            ip_address: pick_option(si.ip_address.clone(), old.ip_address.clone()),
        }
    }
}
