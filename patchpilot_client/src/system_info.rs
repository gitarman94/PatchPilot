use std::collections::HashMap;
use std::net::IpAddr;

use local_ip_address::local_ip;
use serde::Serialize;
use sysinfo::{System, RefreshKind, Networks, Disks};

#[derive(Debug, Serialize, Clone)]
pub struct SystemInfo {
    pub hostname: Option<String>,
    pub ip_address: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,

    pub cpu_brand: Option<String>,
    pub cpu_count: Option<usize>,
    pub architecture: String,

    pub device_type: Option<String>,
    pub device_model: Option<String>,
    pub serial_number: Option<String>,

    pub total_memory: u64,
    pub used_memory: u64,
}

impl SystemInfo {
    pub fn new() -> Self {
        SystemInfo {
            hostname: None,
            ip_address: None,
            os_name: None,
            os_version: None,
            kernel_version: None,

            cpu_brand: None,
            cpu_count: None,
            architecture: std::env::consts::ARCH.to_string(),

            device_type: None,
            device_model: None,
            serial_number: None,

            total_memory: 0,
            used_memory: 0,
        }
    }
}

impl Default for SystemInfo {
    fn default() -> Self {
        SystemInfo::new()
    }
}

/// Gather all system info into a single struct
pub fn get_system_info() -> anyhow::Result<SystemInfo> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let hostname = System::host_name();
    let ip = local_ip().ok().map(|ip| ip.to_string());
    let os_name = System::name();
    let os_version = System::os_version();
    let kernel_version = System::kernel_version();

    let cpu_brand = sys.cpus().get(0).map(|c| c.brand().to_string());
    let cpu_count = Some(sys.cpus().len());

    // Example device info placeholders
    let device_type = Some("unix".into());
    let device_model = Some("unknown".into());
    let serial_number = Some("undefined".into());

    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();

    Ok(SystemInfo {
        hostname,
        ip_address: ip,
        os_name,
        os_version,
        kernel_version,

        cpu_brand,
        cpu_count,
        architecture: std::env::consts::ARCH.to_string(),

        device_type,
        device_model,
        serial_number,

        total_memory,
        used_memory,
    })
}
