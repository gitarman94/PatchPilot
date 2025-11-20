use std::collections::HashMap;
use std::net::IpAddr;

use local_ip_address::local_ip;
use serde::Serialize;
use sysinfo::{System, RefreshKind, CpuExt, DiskExt, NetworkExt};

#[derive(Serialize, Default)]
pub struct SystemInfo {
    #[serde(skip)]
    sys: System,
    #[serde(skip)]
    prev_network: HashMap<String, u64>,

    pub hostname: Option<String>,
    pub ip_address: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub uptime: Option<String>,

    pub cpu_brand: Option<String>,
    pub cpu_count: Option<usize>,
    pub cpu_usage: Option<f32>,

    pub architecture: String,

    pub device_type: Option<String>,
    pub device_model: Option<String>,
    pub serial_number: Option<String>,

    pub total_memory: Option<u64>,
    pub used_memory: Option<u64>,
    pub ram_free: Option<u64>,

    pub disk_total: Option<u64>,
    pub disk_free: Option<u64>,
    pub disk_health: Option<String>,

    pub network_throughput: Option<u64>,
    pub network_interfaces: Option<Vec<String>>,

    pub updates_available: Option<bool>,
}

impl SystemInfo {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(RefreshKind::everything());
        sys.refresh_all();

        // CPU info
        let cpu_brand = sys.cpus().first().map(|c| c.brand().to_string());
        let cpu_count = Some(sys.cpus().len());
        let cpu_usage = if !sys.cpus().is_empty() {
            let sum: f32 = sys.cpus().iter().map(|c| c.cpu_usage()).sum();
            Some(sum / sys.cpus().len() as f32)
        } else {
            None
        };

        // Memory info
        let total_memory = if sys.total_memory() > 0 { Some(sys.total_memory()) } else { None };
        let used_memory = if sys.used_memory() > 0 { Some(sys.used_memory()) } else { None };
        let ram_free = total_memory.and_then(|t| used_memory.map(|u| t.saturating_sub(u)));

        // Disk info
        let mut disk_total = 0;
        let mut disk_free = 0;
        for disk in sys.disks() {
            disk_total += disk.total_space();
            disk_free += disk.available_space();
        }
        let disk_total = if disk_total > 0 { Some(disk_total) } else { None };
        let disk_free = if disk_free > 0 { Some(disk_free) } else { None };
        let disk_health = None; // could be set later with SMART info if available

        // Network info
        let mut network_interfaces = Vec::new();
        let mut network_throughput = 0;
        for (iface, data) in sys.networks() {
            network_interfaces.push(iface.clone());
            let current = data.received() + data.transmitted();
            network_throughput += current;
        }
        let network_interfaces = if network_interfaces.is_empty() { None } else { Some(network_interfaces) };
        let network_throughput = if network_throughput > 0 { Some(network_throughput) } else { None };

        SystemInfo {
            sys,
            prev_network: HashMap::new(),
            hostname: sys.host_name(),
            ip_address: local_ip().ok().map(|ip: IpAddr| ip.to_string()),
            os_name: sys.name(),
            os_version: sys.os_version(),
            kernel_version: sys.kernel_version(),
            uptime: Some(format!("{}s", sys.uptime())),
            cpu_brand,
            cpu_count,
            cpu_usage,
            architecture: std::env::consts::ARCH.to_string(),
            device_type: None,
            device_model: None,
            serial_number: None,
            total_memory,
            used_memory,
            ram_free,
            disk_total,
            disk_free,
            disk_health,
            network_throughput,
            network_interfaces,
            updates_available: None,
        }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_all();

        self.hostname = self.sys.host_name();
        self.ip_address = local_ip().ok().map(|ip| ip.to_string());
        self.os_name = self.sys.name();
        self.os_version = self.sys.os_version();
        self.kernel_version = self.sys.kernel_version();
        self.uptime = Some(format!("{}s", self.sys.uptime()));

        // CPU
        self.cpu_usage = if !self.sys.cpus().is_empty() {
            let sum: f32 = self.sys.cpus().iter().map(|c| c.cpu_usage()).sum();
            Some(sum / self.sys.cpus().len() as f32)
        } else {
            None
        };

        // Memory
        self.total_memory = if self.sys.total_memory() > 0 { Some(self.sys.total_memory()) } else { None };
        self.used_memory = if self.sys.used_memory() > 0 { Some(self.sys.used_memory()) } else { None };
        self.ram_free = self.total_memory.and_then(|t| self.used_memory.map(|u| t.saturating_sub(u)));

        // Disk
        let mut disk_total = 0;
        let mut disk_free = 0;
        for disk in self.sys.disks() {
            disk_total += disk.total_space();
            disk_free += disk.available_space();
        }
        self.disk_total = if disk_total > 0 { Some(disk_total) } else { None };
        self.disk_free = if disk_free > 0 { Some(disk_free) } else { None };

        // Network
        let mut total_throughput = 0u64;
        let mut interfaces = Vec::new();
        for (iface, data) in self.sys.networks() {
            interfaces.push(iface.clone());
            let current = data.received() + data.transmitted();
            let prev = self.prev_network.get(iface).copied().unwrap_or(current);
            total_throughput += current.saturating_sub(prev);
            self.prev_network.insert(iface.clone(), current);
        }
        self.network_throughput = if total_throughput > 0 { Some(total_throughput) } else { None };
        self.network_interfaces = if interfaces.is_empty() { None } else { Some(interfaces) };
    }
}

pub fn get_system_info() -> anyhow::Result<SystemInfo> {
    Ok(SystemInfo::new())
}
