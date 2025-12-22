use serde::{Serialize, Deserialize};
use crate::db;
use diesel::SqliteConnection;

/// Struct for server settings
#[derive(Serialize, Deserialize, Clone)]
pub struct ServerSettings {
    pub auto_approve_devices: bool,
    pub auto_refresh_enabled: bool,
    pub auto_refresh_seconds: i64,
    pub default_action_ttl_seconds: i64,
    pub action_polling_enabled: bool,
    pub ping_target_ip: String,
}

impl ServerSettings {
    pub fn load(conn: &mut SqliteConnection) -> Self {
        db::load_settings(conn).unwrap_or_else(|_| Self::default())
    }

    pub fn save(&self, conn: &mut SqliteConnection) {
        let _ = db::save_settings(conn, self);
    }
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            auto_approve_devices: false,
            auto_refresh_enabled: true,
            auto_refresh_seconds: 30,
            default_action_ttl_seconds: 3600,
            action_polling_enabled: true,
            ping_target_ip: "8.8.8.8".to_string(),
        }
    }
}
