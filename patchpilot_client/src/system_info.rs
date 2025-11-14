use std::collections::HashMap;
use std::net::IpAddr;

use local_ip_address::local_ip;
use sysinfo::{System, RefreshKind, Networks, Disks};

pub struct SystemInfo {
    sys: System,
    prev_network: HashMap<String, u64>,
}

impl SystemInfo {
    pub fn new() -> Self {
        let refresh = RefreshKind::everything();
        let mut sys = System::new_with_specifics(refresh);
        sys.refresh_all();
        SystemInfo {
            sys,
            prev_network: HashMap::new(),
        }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_all();
    }

    pub fn cpu_usage(&mut self) -> f32 {
        // Refresh CPU usage
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
        self.sys.total_memory()
    }

    pub fn ram_used(&self) -> u64 {
        self.sys.used_memory()
    }

    pub fn ram_free(&self) -> u64 {
        self.ram_total().saturating_sub(self.ram_used())
    }

    pub fn disk_usage(&mut self) -> (u64, u64) {
        let mut disks = Disks::new_with_refreshed_list();
        // Refresh disks, removing ones that no longer exist:
        disks.refresh(true);

        let mut total = 0u64;
        let mut free = 0u64;
        for disk in disks.list() {
            total += disk.total_space();
            free += disk.available_space();
        }
        (total, free)
    }

    pub fn network_throughput(&mut self) -> u64 {
        let mut networks = Networks::new_with_refreshed_list();
        networks.refresh(true);

        let mut sum = 0u64;
        for (iface, data) in &networks {
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
