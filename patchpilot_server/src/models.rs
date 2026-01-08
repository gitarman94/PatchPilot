use diesel::prelude::*;
use chrono::{NaiveDateTime, Utc};
use crate::schema::{devices, actions, action_targets, history_log, audit, server_settings};
use serde::{Serialize, Deserialize};

#[derive(Queryable, Identifiable, Selectable, Serialize, Deserialize, Debug)]
#[diesel(table_name = devices)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Device {
    pub id: i64,
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
    pub id: i64,
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
    pub device_id: i64,
    pub system_info: SystemInfo,
    pub device_type: Option<String>,
    pub device_model: Option<String>,
}

#[derive(Debug, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = actions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Action {
    pub id: i64,
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
    pub id: i64,
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
    pub id: i64,
    pub action_id: i64,
    pub device_id: i64,
    pub status: String,
    pub last_update: NaiveDateTime,
    pub response: Option<String>,
}

#[derive(Debug, Insertable, Serialize, Deserialize)]
#[diesel(table_name = action_targets)]
pub struct NewActionTarget {
    pub action_id: i64,
    pub device_id: i64,
    pub status: String,
    pub last_update: NaiveDateTime,
    pub response: Option<String>,
}

impl NewActionTarget {
    pub fn pending(action_id: i64, device_id: i64) -> Self {
        Self {
            action_id,
            device_id,
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
    pub action_id: Option<i64>,
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

#[derive(Debug, Queryable, Identifiable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::server_settings)]
pub struct ServerSettings {
    pub id: i32,
    pub allow_http: bool,
    pub force_https: bool,
    pub max_action_ttl: i64,
    pub max_pending_age: i64,
    pub enable_logging: bool,
    pub default_role: String,
}
