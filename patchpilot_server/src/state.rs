use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};
use sysinfo::System;
use chrono::NaiveDateTime;

use crate::settings::ServerSettings;
use crate::db::DbPool;

pub struct SystemState {
    pub db_pool: DbPool,
    pub system: Arc<Mutex<System>>,
}

impl SystemState {
    /// Example getter that reads system info
    pub fn total_memory(&self) -> u64 {
        let sys = self.system.lock().unwrap();
        sys.total_memory()
    }

    pub fn available_memory(&self) -> u64 {
        let sys = self.system.lock().unwrap();
        sys.available_memory()
    }
}

pub struct AppState {
    pub system: Arc<SystemState>,
    pub pending_devices: Arc<RwLock<HashMap<String, NaiveDateTime>>>,
    pub settings: Arc<RwLock<ServerSettings>>,
}
