use anyhow::Result;
use serde::Serialize;
use sysinfo::{System, SystemExt, CpuExt, DiskExt, NetworkExt, ProcessExt};

#[derive(Serialize)]
pub struct CpuInfo {
    pub name: String,
    pub frequency: u64,
    pub usage: f32,
}

#[derive(Serialize)]
pub struct MemoryInfo {
    pub total: u64,
    pub used: u64,
}

#[derive(Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub total_space: u64,
    pub available_space: u64,
    pub file_system: String,
}

#[derive(Serialize)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub received: u64,
    pub transmitted: u64,
}

#[derive(Serialize)]
pub struct ProcessInfo {
    pub pid: i32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory: u64,
}

#[derive(Serialize)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub hostname: String,
    pub uptime_seconds: u64,
    pub cpus: Vec<CpuInfo>,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub network_interfaces: Vec<NetworkInterfaceInfo>,
    pub processes: Vec<ProcessInfo>,
}

pub fn get_system_info() -> Result<SystemInfo> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpus = sys.cpus().iter().map(|cpu| CpuInfo {
        name: cpu.name().to_string(),
        frequency: cpu.frequency(),
        usage: cpu.cpu_usage(),
    }).collect();

    let memory = MemoryInfo {
        total: sys.total_memory(),
        used: sys.used_memory(),
    };

    let disks = sys.disks().iter().map(|disk| DiskInfo {
        name: disk.name().to_string_lossy().into_owned(),
        total_space: disk.total_space(),
        available_space: disk.available_space(),
        file_system: String::from_utf8_lossy(disk.file_system()).into_owned(),
    }).collect();

    let network_interfaces = sys.networks().iter().map(|(name, data)| NetworkInterfaceInfo {
        name: name.clone(),
        received: data.received(),
        transmitted: data.transmitted(),
    }).collect();

    let processes = sys.processes().iter().map(|(pid, process)| ProcessInfo {
        pid: pid.as_u32() as i32,
        name: process.name().to_string(),
        cpu_usage: process.cpu_usage(),
        memory: process.memory(),
    }).collect();

    Ok(SystemInfo {
        os_name: sys.name().unwrap_or_else(|| "Unknown".to_string()),
        os_version: sys.os_version().unwrap_or_else(|| "Unknown".to_string()),
        kernel_version: sys.kernel_version().unwrap_or_else(|| "Unknown".to_string()),
        hostname: sys.host_name().unwrap_or_else(|| "Unknown".to_string()),
        uptime_seconds: sys.uptime(),
        cpus,
        memory,
        disks,
        network_interfaces,
        processes,
    })
}
