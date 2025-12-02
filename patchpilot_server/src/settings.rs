use serde::{Serialize, Deserialize};
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerSettings {
    pub auto_approve_devices: bool,

    // NEW SETTINGS
    pub auto_refresh_enabled: bool,
    pub auto_refresh_seconds: u64,
}

impl ServerSettings {
    pub fn load() -> Self {
        // Try load JSON → if missing fields, insert defaults
        let loaded: Option<ServerSettings> = fs::read_to_string("settings.json")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());

        match loaded {
            Some(cfg) => Self {
                auto_approve_devices: cfg.auto_approve_devices,

                // If values missing in JSON, fallback to defaults
                auto_refresh_enabled: cfg.auto_refresh_enabled,
                auto_refresh_seconds: if cfg.auto_refresh_seconds == 0 {
                    30
                } else {
                    cfg.auto_refresh_seconds
                },
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
        }
    }
}
