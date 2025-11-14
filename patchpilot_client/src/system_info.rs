use std::collections::HashMap;
use std::net::IpAddr;

use local_ip_address::local_ip;
use sysinfo::{
    Networks, NetworkExt, ProcessorExt, System, SystemExt, 
    RefreshKind, CpuRefreshKind, DiskExt,
};

pub struct SystemInfo {
    sys: System,
    prev_network: HashMap<String, u64>,
}

impl SystemInfo {
    pub fn new() -> Self {
        // Create a System that refreshes everything by default.
        let mut sys = System::new_all();
        sys.refresh_all();
        SystemInfo {
            sys,
            prev_network: HashMap::new(),
        }
    }

    pub fn refresh(&mut self) {
        // Refresh everything
        self.sys.refresh_all();
    }

    pub fn cpu_usage(&mut self) -> f32 {
        // To get correct CPU usage, refresh CPU usage data properly
        self.sys.refresh_cpu_specifics(CpuRefreshKind::everything());
        // Or you can do: self.sys.refresh_cpu_all();
        
        let procs = self.sys.processors();
        if procs.is_empty() {
            0.0
        } else {
            // Sum usage of all processors and average
            let sum: f32 = procs.iter().map(|p| p.cpu_usage()).sum();
            sum / procs.len() as f32
        }
    }

    pub fn ram_total(&self) -> u64 {
        self.sys.total_memory()
    }

    pub fn ram_used(&self) -> u64 {
        self.sys.used_memory()
    }

    pub fn ram_free(&self) -> u64 {
        self.ram_total().saturating_sub(self.ram_used())
    }

    pub fn disk_usage(&mut self) -> (u64, u64) {
        // Make sure disks list is up to date
        self.sys.refresh_disks_list();

        let mut total = 0u64;
        let mut free = 0u64;
        for disk in self.sys.disks() {
            total += disk.get_total_space();
            free += disk.get_available_space();
        }
        (total, free)
    }

    pub fn network_throughput(&mut self) -> u64 {
        // Create networks with refreshed list
        let mut networks = Networks::new();
        networks.refresh(true);  // pass bool: true = remove not listed interfaces

        let mut sum = 0u64;
        for (iface, data) in networks.iter() {
            let current = data.total_received() + data.total_transmitted();
            let prev = self.prev_network.get(iface).copied().unwrap_or(current);
            sum += current.saturating_sub(prev);
            self.prev_network.insert(iface.clone(), current);
        }
        sum
    }

    pub fn ip_address(&self) -> Option<String> {
        local_ip().ok().map(|ip: IpAddr| ip.to_string())
    }

    pub fn hostname(&self) -> Option<String> {
        System::host_name()
    }

    pub fn os_name(&self) -> Option<String> {
        System::name()
    }

    pub fn os_version(&self) -> Option<String> {
        System::os_version()
    }

    pub fn kernel_version(&self) -> Option<String> {
        System::kernel_version()
    }
}
