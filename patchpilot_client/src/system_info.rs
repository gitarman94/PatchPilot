use anyhow::Result;
use serde::Serialize;
use sysinfo::{Disk, NetworkData, Processor, System, SystemExt};  // Updated imports
use local_ip_address::local_ip;

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub mount_point: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub mac: Option<String>,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
    pub errors: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: i32,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatteryInfo {
    pub percentage: Option<f32>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
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
    pub ram_cached: u64,
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

pub fn get_system_info() -> Result<SystemInfo> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // CPU
    let cpu_usage_per_core: Vec<f32> = sys.processors().iter().map(|p| p.cpu_usage()).collect();
    let cpu_usage_total = if cpu_usage_per_core.is_empty() {
        0.0
    } else {
        cpu_usage_per_core.iter().sum::<f32>() / cpu_usage_per_core.len() as f32
    };

    // RAM & swap
    let ram_total = sys.total_memory() / 1024;  // in MB
    let ram_used = sys.used_memory() / 1024;    // in MB
    let ram_free = ram_total - ram_used;
    let ram_cached = sys.used_memory() / 1024;  // Using used_memory as fallback
    let swap_total = sys.total_swap() / 1024;    // in MB
    let swap_used = sys.used_swap() / 1024;      // in MB

    // Disks
    let disks = sys.disks().iter().map(|d| DiskInfo {
        name: d.name().to_string_lossy().to_string(),
        total: d.total_space() / 1024 / 1024,  // Convert to MB
        used: (d.total_space() - d.available_space()) / 1024 / 1024,  // Convert to MB
        free: d.available_space() / 1024 / 1024,  // Convert to MB
        mount_point: d.mount_point().to_string_lossy().to_string(),
    }).collect::<Vec<_>>();

    // Network
    let mut network_interfaces = vec![];
    for (name, data) in sys.networks() {
        network_interfaces.push(NetworkInterfaceInfo {
            name: name.clone(),
            mac: None,  // MAC address can be added if necessary
            received_bytes: data.received(),
            transmitted_bytes: data.transmitted(),
            errors: data.errors(),
        });
    }

    // Top processes by CPU and RAM usage
    let mut processes: Vec<ProcessInfo> = sys.processes().values().map(|p| ProcessInfo {
        pid: p.pid().as_u32() as i32,
        name: p.name().to_string_lossy().to_string(),
        cpu: p.cpu_usage(),
        memory: p.memory() / 1024,  // Convert to MB
    }).collect();

    // Top processes by CPU usage
    processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    let top_processes_cpu = processes.iter().take(5).cloned().collect::<Vec<_>>();

    // Top processes by memory usage
    processes.sort_by(|a, b| b.memory.cmp(&a.memory));
    let top_processes_ram = processes.iter().take(5).cloned().collect::<Vec<_>>();

    // Battery info (platform-specific handling)
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let battery = {
        let output = Command::new("pmset").args(["-g", "batt"]).output().ok();
        if let Some(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().find(|l| l.contains("%")) {
                let parts: Vec<&str> = line.split(';').collect();
                let percentage = parts[0].trim().split_whitespace().nth(1).and_then(|v| v.parse().ok());
                Some(BatteryInfo {
                    percentage,
                    status: Some(parts[1].trim().to_string()),
                })
            } else {
                None
            }
        } else {
            None
        }
    };

    let local_ip = local_ip().ok();

    // Construct system info
    Ok(SystemInfo {
        os_name: sys.name().unwrap_or_else(|| "Unknown".to_string()),
        architecture: sys.architecture().unwrap_or_else(|| "Unknown".to_string()),
        uptime_seconds: sys.uptime(),
        cpu_usage_total,
        cpu_usage_per_core,
        cpu_temperature: None,  // Can be filled if available
        ram_total,
        ram_used,
        ram_free,
        ram_cached,
        swap_total,
        swap_used,
        disks,
        network_interfaces,
        local_ip,
        public_ip: None,  // Needs an external service for public IP
        top_processes_cpu,
        top_processes_ram,
        battery,
    })
}
