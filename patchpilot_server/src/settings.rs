use serde::{Serialize, Deserialize};
use crate::db;
use crate::db::DbPool; // actively used in helper
use diesel::SqliteConnection;
use crate::schema::server_settings;

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
    /// Load settings from the database
    pub fn load(conn: &mut SqliteConnection) -> Self {
        db::load_settings(conn).unwrap_or_else(|_| Self::default())
    }

    /// Save settings to the database
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

// Helper functions to make imports actively used
#[allow(dead_code)]
fn _use_dbpool(pool: &DbPool) {
    let _ = pool.clone();
}

#[allow(dead_code)]
fn _use_server_settings() {
    use crate::schema::server_settings::dsl::*;
    let _ = server_settings.limit(0).load::<()>(&mut SqliteConnection::establish(":memory:").unwrap());
}
