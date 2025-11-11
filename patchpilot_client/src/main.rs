use anyhow::Result;
use serde::Serialize;
use sysinfo::{System, Cpu, Process};

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
    pub mount_point: String,
}

#[derive(Serialize)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub received: u64,
    pub transmitted: u64,
}

#[derive(Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
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

    // ✅ CPUs
    let cpus: Vec<CpuInfo> = sys
        .cpus()
        .iter()
        .map(|cpu: &Cpu| CpuInfo {
            name: cpu.name().to_string(),
            frequency: cpu.frequency(),
            usage: cpu.cpu_usage(),
        })
        .collect();

    // ✅ Memory
    let memory = MemoryInfo {
        total: sys.total_memory(),
        used: sys.used_memory(),
    };

    // ✅ Disks (fixed: removed `.list()`)
    let disks: Vec<DiskInfo> = sys
        .disks()
        .iter()
        .map(|disk| DiskInfo {
            name: disk.name().to_string_lossy().into_owned(),
            total_space: disk.total_space(),
            available_space: disk.available_space(),
            mount_point: disk.mount_point().to_string_lossy().into_owned(),
        })
        .collect();

    // ✅ Networks (fixed: `.networks()` returns an iterable map)
    let network_interfaces: Vec<NetworkInterfaceInfo> = sys
        .networks()
        .iter()
        .map(|(name, data)| NetworkInterfaceInfo {
            name: name.clone(),
            received: data.received(),
            transmitted: data.transmitted(),
        })
        .collect();

    // ✅ Processes
    let processes: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .map(|(pid, process): (&sysinfo::Pid, &Process)| ProcessInfo {
            pid: pid.as_u32(),
            name: process.name().to_string_lossy().into_owned(),
            cpu_usage: process.cpu_usage(),
            memory: process.memory(),
        })
        .collect();

    Ok(SystemInfo {
        os_name: System::name().unwrap_or_else(|| "Unknown".to_string()),
        os_version: System::os_version().unwrap_or_else(|| "Unknown".to_string()),
        kernel_version: System::kernel_version().unwrap_or_else(|| "Unknown".to_string()),
        hostname: System::host_name().unwrap_or_else(|| "Unknown".to_string()),
        uptime_seconds: System::uptime(),
        cpus,
        memory,
        disks,
        network_interfaces,
        processes,
    })
}

fn main() -> Result<()> {
    let info = get_system_info()?;
    println!("{}", serde_json::to_string_pretty(&info)?);
    Ok(())
}
