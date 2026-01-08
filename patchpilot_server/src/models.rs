use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::NaiveDateTime;

use crate::schema::{
    users,
    roles,
    user_roles,
    devices,
    actions,
    action_targets,
    audit,
    server_settings,
};

// User account
#[derive(Debug, Queryable, Identifiable, Serialize)]
#[diesel(table_name = users)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
}

// New user insertable
#[derive(Insertable)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    pub username: &'a str,
    pub password_hash: &'a str,
}

// Role definition
#[derive(Debug, Queryable, Identifiable, Serialize)]
#[diesel(table_name = roles)]
pub struct Role {
    pub id: i32,
    pub name: String,
}

// User-role join
#[derive(Debug, Queryable, Identifiable)]
#[diesel(primary_key(user_id, role_id))]
#[diesel(table_name = user_roles)]
pub struct UserRole {
    pub user_id: i32,
    pub role_id: i32,
}

// Device registered with server
#[derive(Debug, Queryable, Identifiable, Serialize)]
#[diesel(table_name = devices)]
pub struct Device {
    pub id: i32,
    pub hostname: String,
    pub ip_address: String,
    pub approved: bool,
    pub last_seen: NaiveDateTime,
}

// Insertable device
#[derive(Insertable)]
#[diesel(table_name = devices)]
pub struct NewDevice<'a> {
    pub hostname: &'a str,
    pub ip_address: &'a str,
    pub approved: bool,
    pub last_seen: NaiveDateTime,
}

// Action pushed to devices
#[derive(Debug, Queryable, Identifiable, Serialize)]
#[diesel(table_name = actions)]
pub struct Action {
    pub id: String,
    pub name: String,
    pub script: String,
    pub created_at: NaiveDateTime,
    pub expires_at: Option<NaiveDateTime>,
    pub enabled: bool,
}

// Insertable action
#[derive(Insertable)]
#[diesel(table_name = actions)]
pub struct NewAction<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub script: &'a str,
    pub created_at: NaiveDateTime,
    pub expires_at: Option<NaiveDateTime>,
    pub enabled: bool,
}

// Action execution state per device
#[derive(Debug, Queryable, Identifiable, Serialize)]
#[diesel(table_name = action_targets)]
pub struct ActionTarget {
    pub id: i32,
    pub action_id: Option<String>,
    pub device_id: String,
    pub status: String,
    pub last_update: NaiveDateTime,
    pub response: Option<String>,
}

// Insertable action target
#[derive(Insertable)]
#[diesel(table_name = action_targets)]
pub struct NewActionTarget<'a> {
    pub action_id: Option<&'a str>,
    pub device_id: &'a str,
    pub status: &'a str,
    pub last_update: NaiveDateTime,
    pub response: Option<&'a str>,
}

// Audit log entry
#[derive(Debug, Queryable, Identifiable, Serialize)]
#[diesel(table_name = audit)]
pub struct Audit {
    pub id: i32,
    pub actor: String,
    pub action_type: String,
    pub target: Option<String>,
    pub details: Option<String>,
    pub created_at: NaiveDateTime,
}

// Insertable audit record
#[derive(Insertable)]
#[diesel(table_name = audit)]
pub struct NewAudit<'a> {
    pub actor: &'a str,
    pub action_type: &'a str,
    pub target: Option<&'a str>,
    pub details: Option<&'a str>,
    pub created_at: NaiveDateTime,
}

// Diesel-facing server settings row
// Field order must match DB schema exactly
#[derive(Debug, Queryable)]
pub struct ServerSettings {
    pub id: i32,
    pub auto_approve_devices: bool,
    pub auto_refresh_enabled: bool,
    pub auto_refresh_seconds: i64,
    pub default_action_ttl_seconds: i64,
    pub action_polling_enabled: bool,
    pub ping_target_ip: String,
    pub allow_http: bool,
    pub force_https: bool,
}

// Shared enum used by routes and TTL cleanup
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ActionStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl ActionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionStatus::Pending => "pending",
            ActionStatus::Running => "running",
            ActionStatus::Completed => "completed",
            ActionStatus::Failed => "failed",
        }
    }
}
