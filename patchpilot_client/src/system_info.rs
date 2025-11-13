use sysinfo::{System, Cpu, Disk, NetworkData, Process, SystemExt, CpuExt, DiskExt, ProcessExt};

#[derive(Debug)]
pub struct CpuInfo {
    pub name: String,
    pub cores: usize,
}

#[derive(Debug)]
pub struct DiskInfo {
    pub name: String,
    pub total_space: u64,
    pub available_space: u64,
}

#[derive(Debug)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub received: u64,
    pub transmitted: u64,
}

#[derive(Debug)]
pub struct ProcessInfo {
    pub pid: i32,
    pub name: String,
    pub memory: u64,
}

#[derive(Debug, Default)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub hostname: String,
    pub uptime_seconds: u64,
    pub cpus: Vec<CpuInfo>,
    pub disks: Vec<DiskInfo>,
    pub network_interfaces: Vec<NetworkInterfaceInfo>,
    pub processes: Vec<ProcessInfo>,
}

pub fn collect_system_info() -> SystemInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpus = sys.cpus().iter().map(|cpu| CpuInfo {
        name: cpu.brand().to_string(),
        cores: cpu.physical_core_count().unwrap_or(1),
    }).collect();

    let disks = sys.disks().iter().map(|disk| DiskInfo {
        name: disk.name().to_string_lossy().into_owned(),
        total_space: disk.total_space(),
        available_space: disk.available_space(),
    }).collect();

    let network_interfaces = sys.networks().iter().map(|(name, data)| NetworkInterfaceInfo {
        name: name.clone(),
        received: data.received(),
        transmitted: data.transmitted(),
    }).collect();

    let processes = sys.processes().values().map(|process| ProcessInfo {
        pid: process.pid().as_u32() as i32,
        name: process.name().to_string(),
        memory: process.memory(),
    }).collect();

    SystemInfo {
        os_name: sys.name().unwrap_or_else(|| "Unknown".to_string()),
        os_version: sys.os_version().unwrap_or_else(|| "Unknown".to_string()),
        kernel_version: sys.kernel_version().unwrap_or_else(|| "Unknown".to_string()),
        hostname: sys.host_name().unwrap_or_else(|| "Unknown".to_string()),
        uptime_seconds: sys.uptime(),
        cpus,
        disks,
        network_interfaces,
        processes,
    }
}
