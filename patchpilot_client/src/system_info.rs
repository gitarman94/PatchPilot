use sysinfo::{System, SystemExt, CpuExt, DiskExt, NetworkExt};
use std::collections::HashMap;
use std::net::IpAddr;
use gethostname::gethostname;

/// Structs matching `models.rs` SystemInfo expectations
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub hostname: String,
    pub os_name: String,
    pub architecture: String,
    pub cpu: f32,
    pub ram_total: i64,
    pub ram_used: i64,
    pub ram_free: i64,
    pub disk_total: i64,
    pub disk_free: i64,
    pub disk_health: String,
    pub network_throughput: i64,
    pub ping_latency: Option<f32>,
    pub network_interfaces: Option<String>,
    pub ip_address: Option<String>,
}

pub struct SystemInfoCollector {
    sys: System,
    prev_network: HashMap<String, u64>,
}

impl SystemInfoCollector {
    pub fn new() -> Self {
        Self {
            sys: System::new_all(),
            prev_network: HashMap::new(),
        }
    }

    pub fn collect(&mut self) -> SystemInfo {
        // Refresh system info
        self.sys.refresh_all();

        // Hostname
        let hostname = gethostname().to_string_lossy().to_string();

        // OS and architecture
        let os_name = self.sys.name().unwrap_or_else(|| "Unknown".to_string());
        let architecture = std::env::consts::ARCH.to_string();

        // CPU usage (average)
        let cpus = self.sys.cpus();
        let total_cpu = if !cpus.is_empty() {
            cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32
        } else {
            0.0
        };

        // RAM
        let ram_total = self.sys.total_memory() as i64;
        let ram_used = self.sys.used_memory() as i64;
        let ram_free = ram_total - ram_used;

        // Disk stats (sum across all disks)
        let mut disk_total = 0i64;
        let mut disk_free = 0i64;
        for disk in self.sys.disks() {
            disk_total += disk.total_space() as i64;
            disk_free += disk.available_space() as i64;
        }
        let disk_health = "Unknown".to_string();

        // Network throughput
        let mut network_throughput = 0u64;
        let mut iface_names = vec![];
        for (iface, data) in self.sys.networks() {
            let prev = self.prev_network.get(iface).copied().unwrap_or(
                data.received() + data.transmitted()
            );
            let delta = data.received().saturating_add(data.transmitted()).saturating_sub(prev);
            network_throughput = network_throughput.saturating_add(delta);
            self.prev_network.insert(iface.clone(), data.received() + data.transmitted());
            iface_names.push(iface.clone());
        }

        // Optional network interfaces as comma-separated string
        let network_interfaces = if iface_names.is_empty() {
            None
        } else {
            Some(iface_names.join(", "))
        };

        // Optional IP address (pick first non-loopback if available)
        let ip_address = local_ipaddress::get().map(|s| s);

        SystemInfo {
            hostname,
            os_name,
            architecture,
            cpu: total_cpu,
            ram_total,
            ram_used,
            ram_free,
            disk_total,
            disk_free,
            disk_health,
            network_throughput: network_throughput as i64,
            ping_latency: None,
            network_interfaces,
            ip_address,
        }
    }
}

/// Convenience function
pub fn collect_system_info() -> SystemInfo {
    let mut collector = SystemInfoCollector::new();
    collector.collect()
}
