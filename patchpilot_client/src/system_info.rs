use sysinfo::{System, SystemExt, CpuExt, DiskExt, NetworksExt};
use std::collections::HashMap;
use local_ip_address::local_ip;
use std::net::IpAddr;

/// Holds system information and previous network stats
pub struct SystemInfo {
    sys: System,
    prev_network: HashMap<String, u64>,
}

impl SystemInfo {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_all(); // Refresh all data once at start
        Self {
            sys,
            prev_network: HashMap::new(),
        }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_all();
    }

    pub fn cpu_usage(&self) -> f32 {
        let cpus = self.sys.cpus();
        if cpus.is_empty() {
            0.0
        } else {
            cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32
        }
    }

    pub fn ram_total(&self) -> u64 {
        self.sys.total_memory()
    }

    pub fn ram_free(&self) -> u64 {
        self.sys.free_memory()
    }

    pub fn disk_usage(&self) -> (u64, u64) {
        let mut total = 0;
        let mut free = 0;
        for disk in self.sys.disks() {
            total += disk.total_space();
            free += disk.available_space();
        }
        (total, free)
    }

    pub fn network_throughput(&mut self) -> u64 {
        let mut total = 0;
        for (iface, data) in self.sys.networks() {
            let prev = self.prev_network.get(iface).copied().unwrap_or(data.received() + data.transmitted());
            let current = data.received() + data.transmitted();
            total += current - prev;
            self.prev_network.insert(iface.clone(), current);
        }
        total
    }

    pub fn ip_address(&self) -> Option<String> {
        local_ip().ok().map(|ip: IpAddr| ip.to_string())
    }

    pub fn hostname(&self) -> Option<String> {
        self.sys.host_name()
    }

    pub fn os_name(&self) -> String {
        self.sys.name().unwrap_or_else(|| "Unknown".to_string())
    }

    pub fn os_version(&self) -> String {
        self.sys.os_version().unwrap_or_else(|| "Unknown".to_string())
    }

    pub fn architecture(&self) -> String {
        self.sys.kernel_arch().unwrap_or_else(|| "Unknown".to_string())
    }
}
