mod system_info;
mod service;
mod patchpilot_updater;
mod self_update;

use anyhow::Result;
use serde::Serialize;
use sysinfo::{System, RefreshKind, CpuRefreshKind};

use crate::system_info::get_system_info;

/// Information about a single CPU core.
#[derive(Serialize)]
pub struct CpuInfo {
    pub name: String,
    pub frequency: u64,
    pub usage: f32,
}

/// Memory information.
#[derive(Serialize)]
pub struct MemoryInfo {
    pub total: u64,
    pub used: u64,
}

/// Disk information.
#[derive(Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub total_space: u64,
    pub available_space: u64,
    pub mount_point: String,
}

/// Network interface information.
#[derive(Serialize)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub received: u64,
    pub transmitted: u64,
}

/// Process information.
#[derive(Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory: u64,
}

/// System information snapshot (quick local summary).
#[derive(Serialize)]
pub struct LocalSystemInfo {
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

/// Gather system information into a structured object.
pub fn get_local_system_info() -> Result<LocalSystemInfo> {
    let refresh_kind = RefreshKind::everything().with_cpu(CpuRefreshKind::everything());
    let mut sys = System::new_with_specifics(refresh_kind);
    sys.refresh_all();

    // CPU info
    let cpus: Vec<CpuInfo> = sys.cpus().iter().map(|cpu| CpuInfo {
        name: cpu.name().to_string(),
        frequency: cpu.frequency(),
        usage: cpu.cpu_usage(),
    }).collect();

    // Memory info
    let memory = MemoryInfo {
        total: sys.total_memory(),
        used: sys.used_memory(),
    };

    // Disk info
    let disks: Vec<DiskInfo> = sys.disks().iter().map(|disk| DiskInfo {
        name: disk.name().to_string_lossy().to_string(),
        total_space: disk.total_space(),
        available_space: disk.available_space(),
        mount_point: disk.mount_point().to_string_lossy().to_string(),
    }).collect();

    // Network info
    let network_interfaces: Vec<NetworkInterfaceInfo> = sys.networks().iter().map(|(name, data)| NetworkInterfaceInfo {
        name: name.clone(),
        received: data.received(),
        transmitted: data.transmitted(),
    }).collect();

    // Process info
    let processes: Vec<ProcessInfo> = sys.processes().iter().map(|(pid, process)| ProcessInfo {
        pid: pid.as_u32(), // or pid.as_usize() as u32 if needed
        name: process.name().to_string(),
        cpu_usage: process.cpu_usage(),
        memory: process.memory(),
    }).collect();

    Ok(LocalSystemInfo {
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

fn main() -> Result<()> {
    println!("Starting PatchPilot...");

    // Optional: Run self-update before starting
    if let Err(e) = self_update::check_and_update() {
        eprintln!("Self-update check failed: {:?}", e);
    }

    // Local summary logging
    if let Ok(info) = get_local_system_info() {
        println!(
            "System: {} ({}) | Uptime: {}s | {} CPU cores | {:.1}% avg usage",
            info.hostname,
            info.os_name,
            info.uptime_seconds,
            info.cpus.len(),
            info.cpus.iter().map(|c| c.usage).sum::<f32>() / info.cpus.len().max(1) as f32,
        );
    }

    // Full system data
    match get_system_info() {
        Ok(full_info) => {
            println!(
                "Full info gathered: {} disks, {} network interfaces, {} processes",
                full_info.disks.len(),
                full_info.networks.len(),
                full_info.processes.len()
            );
        }
        Err(e) => eprintln!("Error gathering system info: {e}"),
    }

    // Run as background service depending on OS
    #[cfg(unix)]
    {
        println!("Starting PatchPilot service (Unix)...");
        service::run_unix_service()?;
    }

    #[cfg(windows)]
    {
        println!("Starting PatchPilot service (Windows)...");
        service::run_service()?;
    }

    Ok(())
}
