use std::sync::{Arc, RwLock, Mutex};
use std::collections::HashMap;
use sysinfo::System;

use crate::models::DeviceInfo;
use crate::settings::ServerSettings;

pub struct AppState {
    pub system: Mutex<System>,
    pub pending_devices: Arc<RwLock<HashMap<String, DeviceInfo>>>,
    pub settings: Arc<RwLock<ServerSettings>>,
}
