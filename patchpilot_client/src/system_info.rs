use sysinfo::{Networks, System, SystemExt, CpuExt, DiskExt};
use std::collections::HashMap;
use local_ip_address::local_ip;
use std::net::IpAddr;

pub struct SystemInfo {
    sys: System,
    prev_network: HashMap<String, u64>,
}

impl SystemInfo {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        SystemInfo {
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

    pub fn ram_used(&self) -> u64 {
        self.sys.used_memory()
    }

    pub fn ram_free(&self) -> u64 {
        self.ram_total().saturating_sub(self.ram_used())
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
        let mut networks = Networks::new();
        networks.refresh();  // refresh the networks counters

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
