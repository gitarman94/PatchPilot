use serde::{Serialize, Deserialize};
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerSettings {
    pub auto_approve_devices: bool,
}

impl ServerSettings {
    pub fn load() -> Self {
        fs::read_to_string("settings.json")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Self {
                auto_approve_devices: false,
            })
    }

    pub fn save(&self) {
        let _ = fs::write("settings.json", serde_json::to_string_pretty(self).unwrap());
    }
}
