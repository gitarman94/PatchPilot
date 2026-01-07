use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use sysinfo::System;
use crate::models::{ServerSettings};
use crate::db::DbPool;

/// SystemState tracks current system metrics
pub struct SystemState {
    system: RwLock<System>,
}

impl SystemState {
    pub fn new(_pool: DbPool) -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            system: RwLock::new(sys),
        }
    }

    pub fn refresh(&self) {
        let mut sys = self.system.write().unwrap();
        sys.refresh_all();
    }

    pub fn total_memory(&self) -> u64 {
        let sys = self.system.read().unwrap();
        sys.total_memory()
    }

    pub fn available_memory(&self) -> u64 {
        let sys = self.system.read().unwrap();
        sys.available_memory()
    }
}

/// AppState holds server-wide state
pub struct AppState {
    pub system: Arc<SystemState>,
    pub pending_devices: Arc<RwLock<HashMap<String, Instant>>>,
    pub settings: Arc<RwLock<ServerSettings>>,
    pub db_pool: DbPool,
    pub log_audit: Option<Arc<dyn Fn(&mut diesel::SqliteConnection, &str, &str, Option<&str>, Option<&str>) + Send + Sync>>,
}

impl AppState {
    /// Logs an audit event if closure is attached
    pub fn log_audit(
        &self,
        conn: &mut diesel::SqliteConnection,
        actor: &str,
        action_type: &str,
        target: Option<&str>,
        details: Option<&str>,
    ) {
        if let Some(ref f) = self.log_audit {
            f(conn, actor, action_type, target, details);
        }
    }

    /// Registers or updates a pending device heartbeat
    pub fn update_pending_device(&self, device_id: &str) {
        let mut pending = self.pending_devices.write().unwrap();
        pending.insert(device_id.to_string(), Instant::now());
    }

    /// Removes stale pending devices
    pub fn cleanup_stale_devices(&self, max_age_secs: u64) {
        let mut pending = self.pending_devices.write().unwrap();
        let now = Instant::now();
        pending.retain(|_, t| now.duration_since(*t).as_secs() < max_age_secs);
    }
}
