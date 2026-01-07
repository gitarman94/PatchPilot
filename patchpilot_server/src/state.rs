use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};

use chrono::NaiveDateTime;
use sysinfo::System;

use crate::db::DbPool;
use crate::settings::ServerSettings;

pub struct SystemState {
    pub db_pool: DbPool,
    pub system: Arc<Mutex<System>>,
}

impl SystemState {
    pub fn new(db_pool: DbPool) -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        Self {
            db_pool,
            system: Arc::new(Mutex::new(sys)),
        }
    }

    pub fn total_memory(&self) -> u64 {
        self.system.lock().unwrap().total_memory()
    }

    pub fn available_memory(&self) -> u64 {
        self.system.lock().unwrap().available_memory()
    }

    pub fn refresh(&self) {
        self.system.lock().unwrap().refresh_all();
    }
}

pub struct AppState {
    pub system: Arc<SystemState>,
    pub pending_devices: Arc<RwLock<HashMap<String, NaiveDateTime>>>,
    pub settings: Arc<RwLock<ServerSettings>>,
}

impl AppState {
    pub fn new(db_pool: DbPool, settings: ServerSettings) -> Self {
        Self {
            system: Arc::new(SystemState::new(db_pool)),
            pending_devices: Arc::new(RwLock::new(HashMap::new())),
            settings: Arc::new(RwLock::new(settings)),
        }
    }

    pub fn system_info(&self) -> String {
        let sys = &self.system;
        sys.refresh();

        format!(
            "Total Memory: {} MB, Available Memory: {} MB",
            sys.total_memory() / 1024 / 1024,
            sys.available_memory() / 1024 / 1024
        )
    }
}
