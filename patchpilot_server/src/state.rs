// src/state.rs
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use sysinfo::System;

use diesel::prelude::*;
use diesel::result::QueryResult;

use crate::models::ServerSettings;
use crate::db::DbPool;

/// Tracks current system metrics
#[derive(Clone)]
pub struct SystemState {
    system: Arc<RwLock<System>>,
}

impl SystemState {
    pub fn new(_pool: DbPool) -> Arc<Self> {
        let mut sys = System::new_all();
        sys.refresh_all();
        Arc::new(Self {
            system: Arc::new(RwLock::new(sys)),
        })
    }

    pub fn refresh(&self) {
        if let Ok(mut sys) = self.system.write() {
            sys.refresh_all();
        }
    }

    pub fn total_memory(&self) -> u64 {
        if let Ok(sys) = self.system.read() {
            sys.total_memory()
        } else {
            0
        }
    }

    pub fn available_memory(&self) -> u64 {
        if let Ok(sys) = self.system.read() {
            sys.available_memory()
        } else {
            0
        }
    }
}

/// Holds server-wide state
pub struct AppState {
    pub system: Arc<SystemState>,
    pub pending_devices: Arc<RwLock<HashMap<String, Instant>>>,
    pub settings: Arc<RwLock<ServerSettings>>,
    pub db_pool: DbPool,

    /// Optional audit logger closure that returns a Diesel QueryResult<()>
    pub log_audit: Option<
        Arc<
            dyn Fn(
                    &mut diesel::SqliteConnection,
                    &str,
                    &str,
                    Option<&str>,
                    Option<&str>,
                ) -> QueryResult<()>
                + Send
                + Sync,
        >,
    >,
}

impl AppState {
    /// Logs an audit event if closure is attached
    /// returns Ok(()) if logged (or if no closure attached)
    pub fn log_audit(
        &self,
        conn: &mut diesel::SqliteConnection,
        actor: &str,
        action_type: &str,
        target: Option<&str>,
        details: Option<&str>,
    ) -> QueryResult<()> {
        if let Some(ref f) = self.log_audit {
            f(conn, actor, action_type, target, details)
        } else {
            Ok(())
        }
    }

    /// Registers or updates a pending device heartbeat
    pub fn update_pending_device(&self, device_id: &str) {
        if let Ok(mut pending) = self.pending_devices.write() {
            pending.insert(device_id.to_string(), Instant::now());
        }
    }

    /// Removes stale pending devices
    pub fn cleanup_stale_devices(&self, max_age_secs: u64) {
        if let Ok(mut pending) = self.pending_devices.write() {
            let now = Instant::now();
            pending.retain(|_, t| now.duration_since(*t).as_secs() < max_age_secs);
        }
    }
}
