use std::collections::HashMap;
use std::net::IpAddr;

use local_ip_address::local_ip;
use serde::Serialize;
use sysinfo::{System, SystemExt, RefreshKind, Cpu, Disk, NetworkData};

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

    pub ram_total: u64,
    pub ram_used: u64,
    pub ram_free: u64,

    pub disk_total: u64,
    pub disk_free: u64,
    pub disk_health: Option<String>, // Placeholder: could be extended with SMART info

    pub network_throughput: u64,
    pub network_interfaces: Option<String>, // Comma-separated interface names

    pub architecture: String,
    pub device_type: Option<String>,
    pub device_model: Option<String>,
    pub serial_number: Option<String>,
}

impl SystemInfo {
    pub fn new() -> Self {
        let refresh = RefreshKind::new()
            .with_cpu()
            .with_memory()
            .with_disks()
            .with_networks();
        let mut sys = System::new_with_specifics(refresh);
        sys.refresh_all();

        let hostname = sys.host_name();
        let ip_address = local_ip().ok().map(|ip: IpAddr| ip.to_string());
        let os_name = sys.name();
        let os_version = sys.os_version();
        let kernel_version = sys.kernel_version();
        let uptime = Some(format!("{}s", sys.uptime()));

        let cpu_brand = sys.cpus().first().map(|c| c.brand().to_string());
        let cpu_count = Some(sys.cpus().len());
        let cpu_usage = if !sys.cpus().is_empty() {
            Some(sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len() as f32)
        } else {
            None
        };

        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();

        let mut disk_total = 0u64;
        let mut disk_free = 0u64;
        for disk in sys.disks() {
            disk_total += disk.total_space();
            disk_free += disk.available_space();
        }

        let network_interfaces = if sys.networks().is_empty() {
            None
        } else {
            Some(sys.networks().keys().cloned().collect::<Vec<String>>().join(", "))
        };

        SystemInfo {
            sys,
            prev_network: HashMap::new(),
            hostname,
            ip_address,
            os_name,
            os_version,
            kernel_version,
            uptime,
            cpu_brand,
            cpu_count,
            cpu_usage,
            ram_total: total_memory,
            ram_used: used_memory,
            ram_free: total_memory.saturating_sub(used_memory),
            disk_total,
            disk_free,
            disk_health: Some("unknown".into()), // Keep field even if hardware missing
            network_throughput: 0,
            network_interfaces,
            architecture: std::env::consts::ARCH.to_string(),
            device_type: Some("unknown".into()),
            device_model: Some("unknown".into()),
            serial_number: Some("unknown".into()),
        }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_cpu();
        self.sys.refresh_memory();
        self.sys.refresh_disks();
        self.sys.refresh_networks();

        self.hostname = self.sys.host_name();
        self.ip_address = local_ip().ok().map(|ip| ip.to_string());
        self.os_name = self.sys.name();
        self.os_version = self.sys.os_version();
        self.kernel_version = self.sys.kernel_version();
        self.uptime = Some(format!("{}s", self.sys.uptime()));

        self.cpu_usage = if !self.sys.cpus().is_empty() {
            Some(self.sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / self.sys.cpus().len() as f32)
        } else {
            None
        };

        self.ram_total = self.sys.total_memory();
        self.ram_used = self.sys.used_memory();
        self.ram_free = self.ram_total.saturating_sub(self.ram_used);

        self.disk_total = 0;
        self.disk_free = 0;
        for disk in self.sys.disks() {
            self.disk_total += disk.total_space();
            self.disk_free += disk.available_space();
        }

        self.network_interfaces = if self.sys.networks().is_empty() {
            None
        } else {
            Some(self.sys.networks().keys().cloned().collect::<Vec<String>>().join(", "))
        };
    }

    pub fn cpu_usage(&mut self) -> f32 {
        self.sys.refresh_cpu();
        if self.sys.cpus().is_empty() {
            0.0
        } else {
            self.sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / self.sys.cpus().len() as f32
        }
    }

    pub fn disk_usage(&mut self) -> (u64, u64) {
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
        self.sys.refresh_networks();
        let mut sum = 0u64;
        for (iface, data) in self.sys.networks() {
            let current = data.received() + data.transmitted();
            let prev = self.prev_network.get(iface).copied().unwrap_or(current);
            sum += current.saturating_sub(prev);
            self.prev_network.insert(iface.clone(), current);
        }
        self.network_throughput = sum;
        sum
    }
}

pub fn get_system_info() -> anyhow::Result<SystemInfo> {
    Ok(SystemInfo::new())
}
