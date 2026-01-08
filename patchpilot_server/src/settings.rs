use serde::{Serialize, Deserialize};
use diesel::prelude::*;
use diesel::SqliteConnection;
use crate::db;
use crate::models::ServerSettings as ModelServerSettings;
use crate::schema::server_settings;
use diesel::result::QueryResult;

/// Struct for server settings exposed to app
#[derive(Serialize, Deserialize, Clone)]
pub struct ServerSettings {
    pub auto_approve_devices: bool,
    pub auto_refresh_enabled: bool,
    pub auto_refresh_seconds: i64,
    pub default_action_ttl_seconds: i64,
    pub action_polling_enabled: bool,
    pub ping_target_ip: String,

    // HTTPS / HTTP options
    pub force_https: bool,
}

impl ServerSettings {
    /// Load settings from DB, fallback to default
    pub fn load(conn: &mut SqliteConnection) -> Self {
        let s: ModelServerSettings = db::load_settings(conn).unwrap_or_default();

        Self {
            auto_approve_devices: s.auto_approve_devices,
            auto_refresh_enabled: s.auto_refresh_enabled,
            auto_refresh_seconds: s.auto_refresh_seconds,
            default_action_ttl_seconds: s.default_action_ttl_seconds,
            action_polling_enabled: s.action_polling_enabled,
            ping_target_ip: s.ping_target_ip,
            force_https: s.force_https,
        }
    }

    /// Save settings to DB
    pub fn save(&self, conn: &mut SqliteConnection) {
        let s = ModelServerSettings {
            id: 1, // always use the single row ID
            auto_approve_devices: self.auto_approve_devices,
            auto_refresh_enabled: self.auto_refresh_enabled,
            auto_refresh_seconds: self.auto_refresh_seconds,
            default_action_ttl_seconds: self.default_action_ttl_seconds,
            action_polling_enabled: self.action_polling_enabled,
            ping_target_ip: self.ping_target_ip.clone(),
            force_https: self.force_https,
        };

        let _ = db::save_settings(conn, &s);
    }

    /// Update auto-approve and persist
    pub fn set_auto_approve(&mut self, conn: &mut SqliteConnection, value: bool) -> QueryResult<usize> {
        self.auto_approve_devices = value;
        diesel::update(server_settings::table)
            .set(server_settings::auto_approve_devices.eq(value))
            .execute(conn)
    }

    /// Update auto-refresh and persist
    pub fn set_auto_refresh(&mut self, conn: &mut SqliteConnection, value: bool) -> QueryResult<usize> {
        self.auto_refresh_enabled = value;
        diesel::update(server_settings::table)
            .set(server_settings::auto_refresh_enabled.eq(value))
            .execute(conn)
    }

    /// Update auto-refresh interval and persist
    pub fn set_auto_refresh_interval(&mut self, conn: &mut SqliteConnection, value: i64) -> QueryResult<usize> {
        self.auto_refresh_seconds = value;
        diesel::update(server_settings::table)
            .set(server_settings::auto_refresh_seconds.eq(value))
            .execute(conn)
    }

    /// HTTPS / HTTP getters
    pub fn force_https(&self) -> bool {
        self.force_https
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
            force_https: false,
        }
    }
}
