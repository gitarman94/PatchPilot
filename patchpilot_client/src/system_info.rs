use std::collections::HashMap;
use std::net::IpAddr;

use local_ip_address::local_ip;
use serde::Serialize;
use sysinfo::{System, RefreshKind, CpuExt, NetworksExt, DiskExt, SystemExt};

/// SystemInfo for live metrics and reporting
#[derive(Serialize, Default)]
pub struct SystemInfo {
    #[serde(skip)]
    sys: System,
    #[serde(skip)]
    prev_network: HashMap<String, u64>,

    // Serialized fields for sending to server
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
        let refresh = RefreshKind::everything();
        let mut sys = System::new_with_specifics(refresh);
        sys.refresh_all();

        let hostname = sys.host_name();
        let ip_address = local_ip().ok().map(|ip: IpAddr| ip.to_string());
        let os_name = sys.name();
        let os_version = sys.os_version();
        let kernel_version = sys.kernel_version();

        let cpu_brand = sys.cpus().get(0).map(|c| c.brand().to_string());
        let cpu_count = Some(sys.cpus().len());
        let architecture = std::env::consts::ARCH.to_string();

        // Placeholder device info
        let device_type = Some("unknown".into());
        let device_model = Some("unknown".into());
        let serial_number = Some("undefined".into());

        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();

        SystemInfo {
            sys,
            prev_network: HashMap::new(),
            hostname,
            ip_address,
            os_name,
            os_version,
            kernel_version,
            cpu_brand,
            cpu_count,
            architecture,
            device_type,
            device_model,
            serial_number,
            total_memory,
            used_memory,
        }
    }

    /// Refresh system metrics
    pub fn refresh(&mut self) {
        self.sys.refresh_all();
        self.total_memory = self.sys.total_memory();
        self.used_memory = self.sys.used_memory();
        self.hostname = self.sys.host_name();
        self.os_name = self.sys.name();
        self.os_version = self.sys.os_version();
        self.kernel_version = self.sys.kernel_version();
        self.ip_address = local_ip().ok().map(|ip| ip.to_string());
    }

    pub fn cpu_usage(&mut self) -> f32 {
        self.sys.refresh_cpu_all();
        let cpus = self.sys.cpus();
        if cpus.is_empty() {
            0.0
        } else {
            let sum: f32 = cpus.iter().map(|cpu| cpu.cpu_usage()).sum();
            sum / (cpus.len() as f32)
        }
    }

    pub fn ram_total(&self) -> u64 {
        self.total_memory
    }

    pub fn ram_used(&self) -> u64 {
        self.used_memory
    }

    pub fn ram_free(&self) -> u64 {
        self.total_memory.saturating_sub(self.used_memory)
    }

    pub fn disk_usage(&mut self) -> (u64, u64) {
        self.sys.refresh_disks_list();
        self.sys.refresh_disks();

        let mut total = 0u64;
        let mut free = 0u64;
        for disk in self.sys.disks() {
            total += disk.total_space();
            free += disk.available_space();
        }
        (total, free)
    }

    pub fn network_throughput(&mut self) -> u64 {
        self.sys.refresh_networks_list();
        self.sys.refresh_networks();

        let mut sum = 0u64;
        for (iface, data) in self.sys.networks() {
            let current = data.received() + data.transmitted();
            let prev = self.prev_network.get(iface).copied().unwrap_or(current);
            sum += current.saturating_sub(prev);
            self.prev_network.insert(iface.clone(), current);
        }
        sum
    }
}

/// Returns a new SystemInfo instance
pub fn get_system_info() -> anyhow::Result<SystemInfo> {
    Ok(SystemInfo::new())
}
