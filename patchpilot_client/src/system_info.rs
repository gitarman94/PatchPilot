use sysinfo::{System, SystemExt, CpuExt, DiskExt, NetworkExt};
use std::collections::HashMap;

/// Safe wrapper for retrieving system metrics
pub struct SystemInfoCollector {
    sys: System,
    prev_network: HashMap<String, u64>,
}

#[derive(Debug, Clone)]
pub struct CpuStats {
    pub per_core_usage_percent: Vec<f32>,
    pub total_usage_percent: f32,
}

#[derive(Debug, Clone)]
pub struct DiskStats {
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub health: String,
}

#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub throughput_bytes_per_sec: u64,
}

#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub cpu: CpuStats,
    pub ram_total: u64,
    pub ram_used: u64,
    pub ram_free: u64,
    pub disks: Vec<DiskStats>,
    pub network: NetworkStats,
}

impl SystemInfoCollector {
    pub fn new() -> Self {
        let sys = System::new_all();
        Self {
            sys,
            prev_network: HashMap::new(),
        }
    }

    /// Refresh system info, safely handling missing data
    pub fn collect(&mut self) -> SystemMetrics {
        self.sys.refresh_cpu();
        self.sys.refresh_memory();
        self.sys.refresh_disks();
        self.sys.refresh_networks();

        // CPU
        let cpus = self.sys.cpus();
        let per_core_usage: Vec<f32> = cpus.iter().map(|c| c.cpu_usage()).collect();
        let total_cpu_usage = if !per_core_usage.is_empty() {
            per_core_usage.iter().sum::<f32>() / per_core_usage.len() as f32
        } else { 0.0 };

        // RAM
        let total_ram = self.sys.total_memory();
        let used_ram = self.sys.used_memory();
        let free_ram = total_ram.saturating_sub(used_ram);

        // Disks
        let disks: Vec<DiskStats> = self.sys.disks().iter().map(|d| {
            DiskStats {
                total_bytes: d.total_space(),
                free_bytes: d.available_space(),
                health: "Unknown".to_string(), // safe default
            }
        }).collect();

        // Network throughput
        let mut total_throughput = 0u64;
        for (iface, data) in self.sys.networks() {
            let prev = self.prev_network.get(iface).copied().unwrap_or(data.received() + data.transmitted());
            let delta = data.received().saturating_add(data.transmitted()).saturating_sub(prev);
            total_throughput = total_throughput.saturating_add(delta);
            self.prev_network.insert(iface.clone(), data.received() + data.transmitted());
        }

        SystemMetrics {
            cpu: CpuStats {
                per_core_usage_percent: per_core_usage,
                total_usage_percent: total_cpu_usage,
            },
            ram_total: total_ram,
            ram_used: used_ram,
            ram_free: free_ram,
            disks,
            network: NetworkStats {
                throughput_bytes_per_sec: total_throughput,
            },
        }
    }
}
