use serde::Serialize;
use sysinfo::{System, Cpu, Disk, NetworkData, Process, Pid, RefreshKind};
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
    pub errors: u64,
}

/// Process information
#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
}

/// Battery information
#[derive(Debug, Clone, Serialize)]
pub struct BatteryInfo {
    pub percentage: Option<f32>,
    pub status: Option<String>,
}

/// All system information
#[derive(Debug, Clone, Serialize, Default)]
pub struct SystemInfo {
    pub os_name: String,
    pub architecture: String,
    pub uptime_seconds: u64,
    pub cpu_usage_total: f32,
    pub cpu_usage_per_core: Vec<f32>,
    pub cpu_temperature: Option<f32>,
    pub ram_total: u64,
    pub ram_used: u64,
    pub ram_free: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub disks: Vec<DiskInfo>,
    pub network_interfaces: Vec<NetworkInterfaceInfo>,
    pub local_ip: Option<String>,
    pub public_ip: Option<String>,
    pub top_processes_cpu: Vec<ProcessInfo>,
    pub top_processes_ram: Vec<ProcessInfo>,
    pub battery: Option<BatteryInfo>,
}

pub fn get_system_info() -> Result<SystemInfo, Box<dyn std::error::Error>> {
    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu()
            .with_memory()
            .with_disks()
            .with_disks_list()
            .with_networks()
            .with_processes()
    );

    sys.refresh_all();

    // CPU usage per core
    let cpu_usage_per_core: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
    let cpu_usage_total = if !cpu_usage_per_core.is_empty() {
        cpu_usage_per_core.iter().sum::<f32>() / cpu_usage_per_core.len() as f32
    } else {
        0.0
    };

    // Memory
    let ram_total = sys.total_memory() / 1024;
    let ram_used = sys.used_memory() / 1024;
    let ram_free = ram_total.saturating_sub(ram_used);
    let swap_total = sys.total_swap() / 1024;
    let swap_used = sys.used_swap() / 1024;

    // Disks
    let disks: Vec<DiskInfo> = sys.disks().iter().map(|d| DiskInfo {
        name: d.name().to_string_lossy().into_owned(),
        total: d.total_space() / 1024 / 1024,
        used: (d.total_space() - d.available_space()) / 1024 / 1024,
        free: d.available_space() / 1024 / 1024,
        mount_point: d.mount_point().to_string_lossy().into_owned(),
    }).collect();

    // Networks
    let network_interfaces: Vec<NetworkInterfaceInfo> = sys.networks().iter().map(|(name, data)| NetworkInterfaceInfo {
        name: name.clone(),
        mac: None,
        received_bytes: data.received(),
        transmitted_bytes: data.transmitted(),
        errors: 0,
    }).collect();

    // Processes
    let mut process_list: Vec<ProcessInfo> = sys.processes().values().map(|p| ProcessInfo {
        pid: p.pid().as_u32(),
        name: p.name().to_string(),
        cpu: p.cpu_usage(),
        memory: p.memory() / 1024,
    }).collect();

    process_list.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    let top_processes_cpu = process_list.iter().take(5).cloned().collect::<Vec<_>>();

    process_list.sort_by(|a, b| b.memory.cmp(&a.memory));
    let top_processes_ram = process_list.iter().take(5).cloned().collect::<Vec<_>>();

    // Battery (macOS example)
    let battery = {
        #[cfg(target_os = "macos")]
        {
            let output = Command::new("pmset").args(["-g", "batt"]).output().ok();
            if let Some(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = stdout.lines().find(|l| l.contains('%')) {
                    let parts: Vec<&str> = line.split(';').collect();
                    let percentage = parts.get(0)
                        .and_then(|v| v.split_whitespace().nth(1))
                        .and_then(|v| v.trim_end_matches('%').parse::<f32>().ok());
                    Some(BatteryInfo {
                        percentage,
                        status: parts.get(1).map(|s| s.trim().to_string()),
                    })
                } else {
                    None
                }
            } else { None }
        }
        #[cfg(not(target_os = "macos"))]
        { None }
    };

    let local_ip = local_ip().ok().map(|ip| ip.to_string());

    Ok(SystemInfo {
        os_name: sys.name().unwrap_or_else(|| "Unknown".to_string()),
        architecture: sys.os_version().unwrap_or_else(|| "Unknown".to_string()),
        uptime_seconds: sys.uptime(),
        cpu_usage_total,
        cpu_usage_per_core,
        cpu_temperature: None,
        ram_total,
        ram_used,
        ram_free,
        swap_total,
        swap_used,
        disks,
        network_interfaces,
        local_ip,
        public_ip: None,
        top_processes_cpu,
        top_processes_ram,
        battery,
    })
}
