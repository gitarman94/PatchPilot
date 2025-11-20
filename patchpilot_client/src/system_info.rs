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

    pub total_memory: u64,
    pub used_memory: u64,
    pub ram_free: u64,

    pub disk_total: u64,
    pub disk_free: u64,
    pub disk_health: Option<String>,

    pub network_throughput: u64,
    pub network_interfaces: Option<Vec<String>>,

    pub updates_available: Option<bool>,
}

impl SystemInfo {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(RefreshKind::everything());
        sys.refresh_all();

        let hostname = sys.host_name();
        let ip_address = local_ip().ok().map(|ip: IpAddr| ip.to_string());
        let os_name = sys.name();
        let os_version = sys.os_version();
        let kernel_version = sys.kernel_version();
        let uptime = Some(format!("{}s", sys.uptime()));

        let cpu_brand = sys.cpus().first().map(|c| c.brand().to_string());
        let cpu_count = Some(sys.cpus().len());
        let cpu_usage = Some(if !sys.cpus().is_empty() {
            let sum: f32 = sys.cpus().iter().map(|c| c.cpu_usage()).sum();
            sum / sys.cpus().len() as f32
        } else { 0.0 });

        let architecture = std::env::consts::ARCH.to_string();

        let device_type = Some("unknown".into());
        let device_model = Some("unknown".into());
        let serial_number = Some("undefined".into());

        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let ram_free = total_memory.saturating_sub(used_memory);

        let mut disk_total = 0;
        let mut disk_free = 0;
        let mut disk_health = Some("unknown".into());
        for disk in sys.disks() {
            disk_total += disk.total_space();
            disk_free += disk.available_space();
        }

        let mut network_interfaces = Vec::new();
        let mut network_throughput = 0;
        for (iface, data) in sys.networks() {
            network_interfaces.push(iface.clone());
            let current = data.received() + data.transmitted();
            network_throughput += current;
        }
        let network_interfaces = if network_interfaces.is_empty() { None } else { Some(network_interfaces) };

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
            architecture,
            device_type,
            device_model,
            serial_number,
            total_memory,
            used_memory,
            ram_free,
            disk_total,
            disk_free,
            disk_health,
            network_throughput,
            network_interfaces,
            updates_available: Some(false),
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

        self.total_memory = self.sys.total_memory();
        self.used_memory = self.sys.used_memory();
        self.ram_free = self.total_memory.saturating_sub(self.used_memory);

        // CPU
        self.cpu_usage = Some(if !self.sys.cpus().is_empty() {
            let sum: f32 = self.sys.cpus().iter().map(|c| c.cpu_usage()).sum();
            sum / self.sys.cpus().len() as f32
        } else { 0.0 });

        // Disk
        self.disk_total = 0;
        self.disk_free = 0;
        for disk in self.sys.disks() {
            self.disk_total += disk.total_space();
            self.disk_free += disk.available_space();
        }

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
        self.network_throughput = total_throughput;
        self.network_interfaces = if interfaces.is_empty() { None } else { Some(interfaces) };
    }
}

pub fn get_system_info() -> anyhow::Result<SystemInfo> {
    Ok(SystemInfo::new())
}
