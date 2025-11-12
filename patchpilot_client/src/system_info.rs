use serde::Serialize;
use sysinfo::{System, RefreshKind, CpuRefreshKind, Disks, Networks};
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
    // Initialize system with CPU info
    let refresh_kind = RefreshKind::everything().with_cpu(CpuRefreshKind::everything());
    let mut sys = System::new_with_specifics(refresh_kind);
    sys.refresh_all();

    // CPU usage
    let cpu_usage_per_core: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
    let cpu_usage_total = if !cpu_usage_per_core.is_empty() {
        cpu_usage_per_core.iter().sum::<f32>() / cpu_usage_per_core.len() as f32
    } else {
        0.0
    };

    // Memory
    let ram_total = sys.total_memory();
    let ram_used = sys.used_memory();
    let ram_free = ram_total.saturating_sub(ram_used);
    let swap_total = sys.total_swap();
    let swap_used = sys.used_swap();

    // Disks (new API)
    let disks = Disks::new_with_refreshed_list();
    let disks: Vec<DiskInfo> = disks
        .iter()
        .map(|d| DiskInfo {
            name: d.name().to_string_lossy().into_owned(),
            total: d.total_space(),
            used: d.total_space().saturating_sub(d.available_space()),
            free: d.available_space(),
            mount_point: d.mount_point().to_string_lossy().into_owned(),
        })
        .collect();

    // Networks (new API)
    let networks = Networks::new_with_refreshed_list();
    let network_interfaces: Vec<NetworkInterfaceInfo> = networks
        .iter()
        .map(|(name, data)| NetworkInterfaceInfo {
            name: name.clone(),
            mac: None,
            received_bytes: data.received(),
            transmitted_bytes: data.transmitted(),
        })
        .collect();

    // Processes
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

    // Sort for top processes
    process_list.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    let top_processes_cpu = process_list.iter().take(5).cloned().collect::<Vec<_>>();

    process_list.sort_by(|a, b| b.memory.cmp(&a.memory));
    let top_processes_ram = process_list.iter().take(5).cloned().collect::<Vec<_>>();

    // Local IP
    let local_ip = local_ip().ok().map(|ip| ip.to_string());

    // Optional identifiers
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

/// Serial number detection
fn get_serial_number() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/sys/class/dmi/id/product_serial")
            .ok()
            .map(|s| s.trim().to_string())
    }

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("wmic")
            .args(["bios", "get", "serialnumber"])
            .output()
            .ok()?;
        Some(String::from_utf8_lossy(&output.stdout).lines().nth(1)?.trim().to_string())
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ioreg").args(["-l"]).output().ok()?;
        let out_str = String::from_utf8_lossy(&output.stdout);
        for line in out_str.lines() {
            if line.contains("IOPlatformSerialNumber") {
                return Some(line.split('"').nth(3)?.to_string());
            }
        }
        None
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        None
    }
}

/// Device type/model detection
fn get_device_type_model() -> (Option<String>, Option<String>) {
    #[cfg(target_os = "linux")]
    {
        let t =
            std::fs::read_to_string("/sys/class/dmi/id/chassis_type").ok().map(|s| s.trim().to_string());
        let m =
            std::fs::read_to_string("/sys/class/dmi/id/product_name").ok().map(|s| s.trim().to_string());
        (t, m)
    }

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("wmic")
            .args(["computersystem", "get", "model,manufacturer"])
            .output()
            .ok();
        if let Some(output) = output {
            let lines: Vec<_> = String::from_utf8_lossy(&output.stdout).lines().collect();
            if lines.len() >= 2 {
                let parts: Vec<_> = lines[1].split_whitespace().collect();
                if !parts.is_empty() {
                    return (Some(parts[0].to_string()), Some(parts[1..].join(" ")));
                }
            }
        }
        (None, None)
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("system_profiler")
            .args(["SPHardwareDataType"])
            .output()
            .ok();
        if let Some(output) = output {
            let out_str = String::from_utf8_lossy(&output.stdout);
            let mut device_model = None;
            for line in out_str.lines() {
                if line.contains("Model Identifier:") {
                    device_model = Some(line.split(':').nth(1)?.trim().to_string());
                }
            }
            (None, device_model)
        } else {
            (None, None)
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        (None, None)
    }
}
