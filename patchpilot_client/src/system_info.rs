use sysinfo::{System, Disks, Networks, Processes};
use serde::Serialize;
use anyhow::Result;

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

pub fn get_system_info() -> Result<SystemInfo> {
    // Initialize a System instance to collect basic system data
    let mut sys = System::new_all();
    sys.refresh_all();

    // --- Disks ---
    let disks_collection = Disks::new_with_refreshed_list();
    let disks = disks_collection.list().iter().map(|disk| DiskInfo {
        name: disk.name().to_string_lossy().to_string(),
        total_space: disk.total_space(),
        available_space: disk.available_space(),
        mount_point: disk.mount_point().to_string_lossy().to_string(),
    }).collect::<Vec<_>>();

    // --- Processes ---
    let processes = sys.processes().iter().map(|(pid, process)| ProcessInfo {
        pid: pid.as_u32(),
        name: process.name().to_string_lossy().to_string(),
        cpu_usage: process.cpu_usage(),
        memory: process.memory(),
    }).collect::<Vec<_>>();

    // --- Networks ---
    let networks_collection = Networks::new_with_refreshed_list();
    let networks = networks_collection.iter().map(|(name, data)| NetworkInterfaceInfo {
        name: name.clone(),
        received: data.total_received(),
        transmitted: data.total_transmitted(),
    }).collect::<Vec<_>>();

    Ok(SystemInfo {
        device_type: None,
        device_model: None,
        serial_number: None,
        disks,
        processes,
        networks,
    })
}
