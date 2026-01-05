use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};
use sysinfo::{System, SystemExt};
use chrono::NaiveDateTime;
use crate::settings::ServerSettings;
use crate::db::DbPool;

/// Holds system-wide state
pub struct SystemState {
    pub db_pool: DbPool,
    pub system: Arc<Mutex<System>>,
}

impl SystemState {
    /// Create a new SystemState with initialized system info
    pub fn new(db_pool: DbPool) -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        Self {
            db_pool,
            system: Arc::new(Mutex::new(sys)),
        }
    }

    /// Total system memory in bytes
    pub fn total_memory(&self) -> u64 {
        self.system.lock().unwrap().total_memory()
    }

    /// Available system memory in bytes
    pub fn available_memory(&self) -> u64 {
        self.system.lock().unwrap().available_memory()
    }

    /// Refresh system info
    pub fn refresh(&self) {
        self.system.lock().unwrap().refresh_all();
    }
}

/// Holds application-level state
pub struct AppState {
    pub system: Arc<SystemState>,
    pub pending_devices: Arc<RwLock<HashMap<String, NaiveDateTime>>>,
    pub settings: Arc<RwLock<ServerSettings>>,
}

// Example helper function to return system memory info as a string
impl AppState {
    pub fn system_info(&self) -> String {
        let sys_state = self.system.clone();
        sys_state.refresh();
        format!(
            "Total Memory: {} MB, Available Memory: {} MB",
            sys_state.total_memory() / 1024 / 1024,
            sys_state.available_memory() / 1024 / 1024
        )
    }
}
