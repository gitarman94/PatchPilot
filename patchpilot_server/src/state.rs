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

pub struct AppState {
    pub system: Arc<SystemState>,
    pub pending_devices: Arc<RwLock<HashMap<String, NaiveDateTime>>>,
    pub settings: Arc<RwLock<ServerSettings>>,
}
