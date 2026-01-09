// src/settings.rs
//! Thin wrapper around the ServerSettings model + DB helpers.
//! Exports `ServerSettings` publicly so other modules can `use crate::settings::ServerSettings`.

use diesel::sqlite::SqliteConnection;
use diesel::result::QueryResult;

pub use crate::models::ServerSettings;
use crate::db::{load_settings, save_settings, ServerSettingsRow};

impl ServerSettings {
    /// Load settings from DB and convert into the app-level ServerSettings struct.
    /// Panics only if DB access itself fails (mirrors previous code behavior).
    pub fn load(conn: &mut SqliteConnection) -> ServerSettings {
        // load_settings returns a ServerSettingsRow (creates default if missing)
        let row: ServerSettingsRow = load_settings(conn)
            .expect("Failed to load server settings from DB");

        ServerSettings {
            id: row.id,
            auto_approve_devices: row.auto_approve_devices,
            auto_refresh_enabled: row.auto_refresh_enabled,
            auto_refresh_seconds: row.auto_refresh_seconds,
            default_action_ttl_seconds: row.default_action_ttl_seconds,
            action_polling_enabled: row.action_polling_enabled,
            ping_target_ip: row.ping_target_ip,
            force_https: row.force_https,
        }
    }

    /// Persist the current ServerSettings back into the DB (replaces the single-row).
    pub fn save(&self, conn: &mut SqliteConnection) -> QueryResult<()> {
        let row = ServerSettingsRow {
            id: self.id,
            auto_approve_devices: self.auto_approve_devices,
            auto_refresh_enabled: self.auto_refresh_enabled,
            auto_refresh_seconds: self.auto_refresh_seconds,
            default_action_ttl_seconds: self.default_action_ttl_seconds,
            action_polling_enabled: self.action_polling_enabled,
            ping_target_ip: self.ping_target_ip.clone(),
            force_https: self.force_https,
        };
        save_settings(conn, &row)
    }

    /// Convenience mutator that persists the `auto_approve_devices` flag.
    pub fn set_auto_approve(&mut self, conn: &mut SqliteConnection, v: bool) -> QueryResult<()> {
        self.auto_approve_devices = v;
        self.save(conn)
    }

    /// Convenience mutator that persists the `auto_refresh_enabled` flag.
    pub fn set_auto_refresh(&mut self, conn: &mut SqliteConnection, v: bool) -> QueryResult<()> {
        self.auto_refresh_enabled = v;
        self.save(conn)
    }

    /// Convenience mutator that persists the `auto_refresh_seconds` interval.
    pub fn set_auto_refresh_interval(&mut self, conn: &mut SqliteConnection, v: i64) -> QueryResult<()> {
        self.auto_refresh_seconds = v;
        self.save(conn)
    }
}
