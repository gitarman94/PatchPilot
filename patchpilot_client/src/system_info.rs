use serde::Serialize;
use sysinfo::{System, SystemExt, CpuRefreshKind, RefreshKind, DiskExt, NetworkExt, ProcessExt};
use local_ip_address::local_ip;

#[cfg(any(target_os = "windows", target_os = "macos"))]
use std::process::Command;

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
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
}

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
    let refresh_kind = RefreshKind::everything().with_cpu(CpuRefreshKind::everything());
    let mut sys = System::new_with_specifics(refresh_kind);
    sys.refresh_all();

    // CPU
    let cpu_usage_per_core: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
    let cpu_usage_total = cpu_usage_per_core.iter().copied().sum::<f32>() / cpu_usage_per_core.len().max(1) as f32;

    // Memory
    let ram_total = sys.total_memory();
    let ram_used = sys.used_memory();
    let ram_free = ram_total.saturating_sub(ram_used);
    let swap_total = sys.total_swap();
    let swap_used = sys.used_swap();

    // Disks
    let disks: Vec<DiskInfo> = sys
        .disks()
        .iter()
        .map(|d| DiskInfo {
            name: d.name().to_string_lossy().into_owned(),
            total: d.total_space(),
            used: d.total_space().saturating_sub(d.available_space()),
            free: d.available_space(),
            mount_point: d.mount_point().to_string_lossy().into_owned(),
        })
        .collect();

    // Networks
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

    // Processes
    let mut processes: Vec<ProcessInfo> = sys
        .processes()
        .values()
        .map(|p| ProcessInfo {
            pid: p.pid().as_u32(),
            name: p.name().to_string(),
            cpu: p.cpu_usage(),
            memory: p.memory(),
        })
        .collect();

    processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    let top_processes_cpu = processes.iter().take(5).cloned().collect();

    processes.sort_by(|a, b| b.memory.cmp(&a.memory));
    let top_processes_ram = processes.iter().take(5).cloned().collect();

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

// --- Serial number and device info ---
#[cfg(target_os = "linux")]
fn get_serial_number() -> Option<String> {
    std::fs::read_to_string("/sys/class/dmi/id/product_serial").ok().map(|s| s.trim().to_string())
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn get_serial_number() -> Option<String> {
    if cfg!(target_os = "windows") {
        let output = Command::new("wmic")
            .args(["bios", "get", "serialnumber"])
            .output()
            .ok()?;
        return Some(String::from_utf8_lossy(&output.stdout).lines().nth(1)?.trim().to_string());
    }
    if cfg!(target_os = "macos") {
        let output = Command::new("ioreg").args(["-l"]).output().ok()?;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.contains("IOPlatformSerialNumber") {
                return line.split('"').nth(3).map(|s| s.to_string());
            }
        }
    }
    None
}

fn get_device_type_model() -> (Option<String>, Option<String>) {
    #[cfg(target_os = "linux")]
    {
        let t = std::fs::read_to_string("/sys/class/dmi/id/chassis_type").ok().map(|s| s.trim().to_string());
        let m = std::fs::read_to_string("/sys/class/dmi/id/product_name").ok().map(|s| s.trim().to_string());
        (t, m)
    }
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("wmic")
            .args(["computersystem", "get", "model,manufacturer"])
            .output()
            .ok()?;
        let lines: Vec<_> = String::from_utf8_lossy(&output.stdout).lines().collect();
        if lines.len() >= 2 {
            let parts: Vec<_> = lines[1].split_whitespace().collect();
            if !parts.is_empty() {
                return (Some(parts[0].to_string()), Some(parts[1..].join(" ")));
            }
        }
        (None, None)
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("system_profiler").args(["SPHardwareDataType"]).output().ok()?;
        let out_str = String::from_utf8_lossy(&output.stdout);
        let device_model = out_str
            .lines()
            .find(|l| l.contains("Model Identifier:"))
            .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string());
        (None, device_model)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        (None, None)
    }
}
