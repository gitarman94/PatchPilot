use sysinfo::{System, SystemExt, CpuExt, DiskExt, NetworkData};
use std::collections::HashMap;
use std::net::IpAddr;
use local_ip_address::local_ip;
use log::warn;

/// Structure to hold system info and previous network stats
pub struct SystemInfo {
    sys: System,
    prev_network: HashMap<String, u64>,
}

impl SystemInfo {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_all(); // refresh everything
        SystemInfo {
            sys,
            prev_network: HashMap::new(),
        }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_all();
    }

    pub fn cpu_usage(&self) -> f32 {
        // Sum all CPU usage
        self.sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / self.sys.cpus().len() as f32
    }

    pub fn total_memory(&self) -> u64 {
        self.sys.total_memory()
    }

    pub fn free_memory(&self) -> u64 {
        self.sys.free_memory()
    }

    pub fn disk_usage(&self) -> (u64, u64) {
        let (total, free) = self.sys.disks().iter().fold((0, 0), |acc, d| {
            (acc.0 + d.total_space(), acc.1 + d.available_space())
        });
        (total, free)
    }

    pub fn network_usage(&mut self) -> u64 {
        let mut total_transferred = 0u64;

        for (iface, data) in self.sys.networks() {
            let prev = self.prev_network.get(iface).copied().unwrap_or(0);
            let current = data.received() + data.transmitted();
            total_transferred += current - prev;
            self.prev_network.insert(iface.clone(), current);
        }

        total_transferred
    }

    pub fn os_name(&self) -> String {
        sysinfo::System::name().unwrap_or_else(|| "Unknown".to_string())
    }

    pub fn os_version(&self) -> String {
        sysinfo::System::os_version().unwrap_or_else(|| "Unknown".to_string())
    }

    pub fn hostname(&self) -> Option<String> {
        local_ip().ok().map(|ip: IpAddr| ip.to_string())
    }
}
