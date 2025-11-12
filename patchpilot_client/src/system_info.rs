use sysinfo::{System, Process, Disk, NetworkData};
use serde::Serialize;

#[derive(Debug, Serialize, Default)]
pub struct DiskInfo {
    pub name: String,
    pub total_space: u64,
    pub available_space: u64,
    pub mount_point: String,
}

#[derive(Debug, Serialize, Default)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory: u64,
}

#[derive(Debug, Serialize, Default)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub received: u64,
    pub transmitted: u64,
}

#[derive(Debug, Serialize, Default)]
pub struct SystemInfo {
    pub device_type: Option<String>,
    pub device_model: Option<String>,
    pub serial_number: Option<String>,
    pub disks: Vec<DiskInfo>,
    pub processes: Vec<ProcessInfo>,
    pub networks: Vec<NetworkInterfaceInfo>,
}

pub fn get_system_info() -> anyhow::Result<SystemInfo> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // --- Disks ---
    let disks = sys.disks().iter().map(|disk: &Disk| DiskInfo {
        name: disk.name().to_string_lossy().to_string(),
        total_space: disk.total_space(),
        available_space: disk.available_space(),
        mount_point: disk.mount_point().to_string_lossy().to_string(),
    }).collect::<Vec<_>>();

    // --- Processes ---
    let processes = sys.processes().iter().map(|(pid, process): (&sysinfo::Pid, &Process)| ProcessInfo {
        pid: pid.as_u32(),
        name: process.name().to_string_lossy().to_string(),
        cpu_usage: process.cpu_usage(),
        memory: process.memory(),
    }).collect::<Vec<_>>();

    // --- Network Interfaces ---
    let networks = sys.networks().iter().map(|(name, data): (&String, &NetworkData)| NetworkInterfaceInfo {
        name: name.clone(),
        received: data.received(),
        transmitted: data.transmitted(),
    }).collect::<Vec<_>>();

    // --- Device info placeholders ---
    let device_type = Some("unknown".to_string());
    let device_model = Some("unknown".to_string());
    let serial_number = Some("unknown".to_string());

    Ok(SystemInfo {
        device_type,
        device_model,
        serial_number,
        disks,
        processes,
        networks,
    })
}
