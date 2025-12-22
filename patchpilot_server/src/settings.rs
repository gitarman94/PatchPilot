use serde::{Serialize, Deserialize};
use diesel::prelude::*;
use diesel::SqliteConnection;
use crate::db;
use crate::schema::server_settings;
use diesel::result::QueryResult;

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
    /// Load settings from DB, fallback to default
    pub fn load(conn: &mut SqliteConnection) -> Self {
        db::load_settings(conn).unwrap_or_else(|_| Self::default())
    }

    /// Save settings to DB
    pub fn save(&self, conn: &mut SqliteConnection) {
        let _ = db::save_settings(conn, self);
    }

    /// Update a single field and persist immediately
    pub fn set_auto_approve(&mut self, conn: &mut SqliteConnection, value: bool) -> QueryResult<usize> {
        self.auto_approve_devices = value;
        diesel::update(server_settings::table)
            .set(server_settings::auto_approve_devices.eq(value))
            .execute(conn)
    }

    pub fn set_auto_refresh(&mut self, conn: &mut SqliteConnection, value: bool) -> QueryResult<usize> {
        self.auto_refresh_enabled = value;
        diesel::update(server_settings::table)
            .set(server_settings::auto_refresh_enabled.eq(value))
            .execute(conn)
    }

    pub fn set_auto_refresh_interval(&mut self, conn: &mut SqliteConnection, value: i64) -> QueryResult<usize> {
        self.auto_refresh_seconds = value;
        diesel::update(server_settings::table)
            .set(server_settings::auto_refresh_seconds.eq(value))
            .execute(conn)
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