use serde::{Serialize, Deserialize};
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerSettings {
    pub auto_approve_devices: bool,

    pub auto_refresh_enabled: bool,
    pub auto_refresh_seconds: u64,

    // NEW OPTIONAL SETTINGS
    pub default_action_ttl_seconds: u64,
    pub action_polling_enabled: bool,
}

impl ServerSettings {
    pub fn load() -> Self {
        let loaded: Option<ServerSettings> = fs::read_to_string("settings.json")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());

        match loaded {
            Some(cfg) => Self {
                auto_approve_devices: cfg.auto_approve_devices,
                auto_refresh_enabled: cfg.auto_refresh_enabled,
                auto_refresh_seconds: if cfg.auto_refresh_seconds == 0 { 30 } else { cfg.auto_refresh_seconds },

                default_action_ttl_seconds: if cfg.default_action_ttl_seconds == 0 { 3600 } else { cfg.default_action_ttl_seconds },
                action_polling_enabled: cfg.action_polling_enabled,
            },

            None => Self::default(),
        }
    }

    pub fn save(&self) {
        let _ = fs::write(
            "settings.json",
            serde_json::to_string_pretty(self).unwrap(),
        );
    }
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            auto_approve_devices: false,

            auto_refresh_enabled: true,
            auto_refresh_seconds: 30,

            default_action_ttl_seconds: 3600, // 1 hour TTL
            action_polling_enabled: true,
        }
    }
}
