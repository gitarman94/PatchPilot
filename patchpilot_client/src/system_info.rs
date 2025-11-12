use serde::Serialize;
use sysinfo::{System, Disk, Process}; // import only what you need
use sysinfo::NetworkExt;  // if networks API exposed via trait
use local_ip_address::local_ip;
use std::process::Command;

/// Disk information
#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub mount_point: String,
}

/// Network interface information
#[derive(Debug, Clone, Serialize)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub mac: Option<String>,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
}

/// Process information
#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
}

/// Full system snapshot
#[derive(Debug, Clone, Serialize, Default)]
pub struct SystemInfo {
    pub os_name: String,
    pub architecture: String,
    pub uptime_seconds: u64,
    pub cpu_usage_total: f32,
    pub cpu_usage_per_core: Vec<f32>,
    pub ram_total: u64,
    pub ram_used: u64,
    pub ram_free: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub disks: Vec<DiskInfo>,
    pub network_interfaces: Vec<NetworkInterfaceInfo>,
    pub local_ip: Option<String>,
    pub top_processes_cpu: Vec<ProcessInfo>,
    pub top_processes_ram: Vec<ProcessInfo>,
    pub serial_number: Option<String>,
    pub device_type: Option<String>,
    pub device_model: Option<String>,
}

pub fn get_system_info() -> Result<SystemInfo, Box<dyn std::error::Error>> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_usage_per_core: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
    let cpu_usage_total = if !cpu_usage_per_core.is_empty() {
        cpu_usage_per_core.iter().sum::<f32>() / cpu_usage_per_core.len() as f32
    } else {
        0.0
    };

    let ram_total = sys.total_memory();
    let ram_used = sys.used_memory();
    let ram_free = ram_total.saturating_sub(ram_used);
    let swap_total = sys.total_swap();
    let swap_used = sys.used_swap();

    let disks: Vec<DiskInfo> = sys
        .disks()  // If this doesn't compile, you may need to use `sys.disks_list()` or similar
        .iter()
        .map(|d| DiskInfo {
            name: d.name().to_string_lossy().into_owned(),
            total: d.total_space(),
            used: d.total_space().saturating_sub(d.available_space()),
            free: d.available_space(),
            mount_point: d.mount_point().to_string_lossy().into_owned(),
        })
        .collect();

    let network_interfaces: Vec<NetworkInterfaceInfo> = sys
        .networks()
        .iter()
        .map(|(name, data)| NetworkInterfaceInfo {
            name: name.clone(),
            mac: None,
            received_bytes: data.received(),
            transmitted_bytes: data.transmitted(),
        })
        .collect();

    let mut process_list: Vec<ProcessInfo> = sys
        .processes()
        .values()
        .map(|p| ProcessInfo {
            pid: p.pid().as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            cpu: p.cpu_usage(),
            memory: p.memory(),
        })
        .collect();

    process_list.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    let top_processes_cpu = process_list.iter().take(5).cloned().collect::<Vec<_>>();

    process_list.sort_by(|a, b| b.memory.cmp(&a.memory));
    let top_processes_ram = process_list.iter().take(5).cloned().collect::<Vec<_>>();

    let local_ip = local_ip().ok().map(|ip| ip.to_string());

    let serial_number = get_serial_number();
    let (device_type, device_model) = get_device_type_model();

    Ok(SystemInfo {
        os_name: System::name().unwrap_or_else(|| "Unknown".to_string()),
        architecture: System::kernel_version().unwrap_or_else(|| "Unknown".to_string()),
        uptime_seconds: System::uptime(),
        cpu_usage_total,
        cpu_usage_per_core,
        ram_total,
        ram_used,
        ram_free,
        swap_total,
        swap_used,
        disks,
        network_interfaces,
        local_ip,
        top_processes_cpu,
        top_processes_ram,
        serial_number,
        device_type,
        device_model,
    })
}
