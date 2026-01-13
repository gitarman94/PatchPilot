use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use chrono::{NaiveDateTime, Utc};
use serde::{Serialize, Deserialize};

use crate::schema::{
    devices, actions, action_targets, history_log, audit, users, roles, user_roles, user_groups,
    groups, server_settings,
};

#[derive(Debug, Queryable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = devices)]
pub struct Device {
    pub id: i64,
    pub device_id: i64,
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

#[derive(Debug, Insertable, AsChangeset, Serialize, Deserialize)]
#[diesel(table_name = devices)]
pub struct NewDevice {
    pub device_id: i64,
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

impl Device {
    pub fn all(conn: &mut SqliteConnection) -> QueryResult<Vec<Device>> {
        devices::table.load::<Device>(conn)
    }

    pub fn find_by_device_id(conn: &mut SqliteConnection, device_id_param: i64) -> QueryResult<Device> {
        devices::table
            .filter(devices::device_id.eq(device_id_param))
            .first::<Device>(conn)
    }

    pub fn find_by_id(conn: &mut SqliteConnection, id_param: i64) -> QueryResult<Device> {
        devices::table
            .filter(devices::id.eq(id_param))
            .first::<Device>(conn)
    }
}

#[derive(Debug, Queryable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = actions)]
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
    pub action_type: String,
    pub parameters: Option<String>,
    pub author: Option<String>,
    pub created_at: NaiveDateTime,
    pub expires_at: NaiveDateTime,
    pub canceled: bool,
}

impl Action {
    pub fn all(conn: &mut SqliteConnection) -> QueryResult<Vec<Action>> {
        actions::table.load::<Action>(conn)
    }
}

#[derive(Debug, Queryable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = action_targets)]
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

#[derive(Debug, Queryable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = history_log)]
pub struct HistoryEntry {
    pub id: i64,
    pub action_id: i64,
    pub device_name: Option<String>,
    pub actor: Option<String>,
    pub action_type: String,
    pub details: Option<String>,
    pub created_at: NaiveDateTime,
}

impl HistoryEntry {
    pub fn all(conn: &mut SqliteConnection) -> QueryResult<Vec<HistoryEntry>> {
        history_log::table.load::<HistoryEntry>(conn)
    }
}

#[derive(Debug, Queryable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = audit)]
pub struct AuditLog {
    pub id: i32,
    pub actor: String,
    pub action_type: String,
    pub target: Option<String>,
    pub details: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Queryable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = users)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
    pub created_at: NaiveDateTime,
}

impl User {
    pub fn all(conn: &mut SqliteConnection) -> QueryResult<Vec<User>> {
        users::table.load::<User>(conn)
    }

    // Check if user has a role (compare by role name)
    pub fn has_role(&self, role_name: &str) -> bool {
        // TODO: implement real DB join if needed; placeholder for now
        self.role_name() == role_name
    }

    fn role_name(&self) -> &str {
        // Placeholder: adjust with actual DB query if needed
        "Admin"
    }
}

#[derive(Debug, Queryable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = roles)]
pub struct Role {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Queryable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(User))]
#[diesel(belongs_to(Role))]
#[diesel(table_name = user_roles)]
pub struct UserRole {
    pub id: i32,
    pub user_id: i32,
    pub role_id: i32,
}

impl UserRole {
    pub const ADMIN: &'static str = "Admin";
}

#[derive(Debug, Queryable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(User))]
#[diesel(belongs_to(Group))]
#[diesel(table_name = user_groups)]
pub struct UserGroup {
    pub id: i32,
    pub user_id: i32,
    pub group_id: i32,
}

#[derive(Debug, Queryable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = groups)]
pub struct Group {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Queryable, Identifiable, Serialize, Deserialize, Clone)]
#[diesel(table_name = server_settings)]
pub struct ServerSettings {
    pub id: i32,
    pub auto_approve_devices: bool,
    pub auto_refresh_enabled: bool,
    pub auto_refresh_seconds: i64,
    pub default_action_ttl_seconds: i64,
    pub action_polling_enabled: bool,
    pub ping_target_ip: String,
    pub force_https: bool,
}

impl UserRole {
    /// Checks if this UserRole has an admin role
    pub fn is_admin(&self, conn: &mut SqliteConnection) -> bool {
        use crate::schema::roles::dsl::*;
        if let Ok(role) = roles.filter(id.eq(self.role_id)).first::<Role>(conn) {
            return role.name.to_lowercase() == "admin";
        }
        false
    }
}
