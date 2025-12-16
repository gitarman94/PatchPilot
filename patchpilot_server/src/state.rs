use std::sync::{Arc, RwLock, Mutex};
use std::collections::HashMap;
use sysinfo::System;

use crate::models::DeviceInfo;
use crate::settings::ServerSettings;

#[derive(Clone)]
pub struct AppState {
    pub system: Arc<Mutex<System>>,
    pub pending_devices: Arc<RwLock<HashMap<String, DeviceInfo>>>,
    pub settings: Arc<RwLock<ServerSettings>>,
}
