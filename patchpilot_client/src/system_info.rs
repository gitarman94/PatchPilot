use sysinfo::{CpuExt, DiskExt, NetworkExt, System, SystemExt};
use std::collections::HashMap;
use local_ip_address::local_ip;
use gethostname::gethostname;

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub architecture: String,
    pub cpu: f32,
    pub ram_total: u64,
    pub ram_used: u64,
    pub ram_free: u64,
    pub disk_total: u64,
    pub disk_free: u64,
    pub disk_health: String,
    pub network_throughput: u64,
    pub hostname: String,
    pub ip_address: Option<String>,
}

pub struct SystemInfoCollector {
    sys: System,
    prev_network: HashMap<String, u64>,
}

impl SystemInfoCollector {
    pub fn new() -> Self {
        let sys = System::new_all();
        Self {
            sys,
            prev_network: HashMap::new(),
        }
    }

    pub fn collect(&mut self) -> SystemInfo {
        self.sys.refresh_cpu();
        self.sys.refresh_memory();
        self.sys.refresh_disks();
        self.sys.refresh_networks();

        // CPU usage (average)
        let cpu_usage = self.sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>()
            / self.sys.cpus().len().max(1) as f32;

        // RAM
        let ram_total = self.sys.total_memory();
        let ram_used = self.sys.used_memory();
        let ram_free = ram_total.saturating_sub(ram_used);

        // Disks
        let (disk_total, disk_free) = self.sys.disks().iter().fold((0, 0), |acc, d| {
            (acc.0 + d.total_space(), acc.1 + d.available_space())
        });

        // Network throughput
        let mut network_throughput = 0u64;
        for (iface, data) in self.sys.networks() {
            let prev = self.prev_network.get(iface).copied().unwrap_or(data.received() + data.transmitted());
            let delta = data.received() + data.transmitted() - prev;
            network_throughput += delta;
            self.prev_network.insert(iface.clone(), data.received() + data.transmitted());
        }

        // OS info
        let os_name = self.sys.name().unwrap_or_else(|| "Unknown".to_string());
        let os_version = self.sys.os_version().unwrap_or_else(|| "Unknown".to_string());
        let architecture = std::env::consts::ARCH.to_string();

        // Hostname and IP
        let hostname = gethostname().to_string_lossy().to_string();
        let ip_address = local_ip().ok();

        SystemInfo {
            os_name,
            os_version,
            architecture,
            cpu: cpu_usage,
            ram_total,
            ram_used,
            ram_free,
            disk_total,
            disk_free,
            disk_health: "Unknown".to_string(),
            network_throughput,
            hostname,
            ip_address,
        }
    }
}

pub fn collect_system_info() -> SystemInfo {
    let mut collector = SystemInfoCollector::new();
    collector.collect()
}
