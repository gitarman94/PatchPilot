use diesel::prelude::*;
use chrono::{NaiveDateTime, Utc};
use rocket::serde::{Serialize, Deserialize};
use crate::schema::{devices, actions, action_targets, history_log, audit, server_settings};

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
    pub uptime: Option<i64>,
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
    pub uptime: Option<i64>,
    pub updates_available: bool,
    pub network_interfaces: Option<String>,
    pub ip_address: Option<String>,
}

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

#[derive(Debug, Queryable, Identifiable, Selectable, Serialize, Deserialize)]
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

#[derive(Debug, Insertable, Serialize, Deserialize)]
#[diesel(table_name = action_targets)]
pub struct NewActionTarget {
    pub action_id: String,
    pub device_id: String,
    pub status: String,
    pub last_update: NaiveDateTime,
    pub response: Option<String>,
}

impl NewActionTarget {
    pub fn pending(action_id: &str, device_id: &str) -> Self {
        Self {
            action_id: action_id.to_string(),
            device_id: device_id.to_string(),
            status: "Pending".to_string(),
            last_update: Utc::now().naive_utc(),
            response: None,
        }
    }
}

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

#[derive(Debug, Queryable, Selectable, Serialize, Deserialize, Default)]
#[diesel(table_name = server_settings)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ServerSettings {
    pub allow_http: bool,
    pub force_https: bool,
    pub id: i32,
    pub auto_approve_devices: bool,
    pub auto_refresh_enabled: bool,
    pub auto_refresh_seconds: i64,
    pub default_action_ttl_seconds: i64,
    pub action_polling_enabled: bool,
    pub ping_target_ip: String,
}
