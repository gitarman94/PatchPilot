use sysinfo::{System, SystemExt, ProcessExt, DiskExt, NetworksExt, NetworkExt};
use serde::Serialize;
use std::default::Default;

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

pub fn get_system_info() -> Result<SystemInfo, anyhow::Error> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // --- Disks ---
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

    // --- Processes ---
    let processes: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .map(|(&pid, process)| ProcessInfo {
            pid: pid.as_u32(),
            name: process.name().to_string(),
            cpu_usage: process.cpu_usage(),
            memory: process.memory(),
        })
        .collect();

    // --- Network Interfaces ---
    let networks: Vec<NetworkInterfaceInfo> = sys
        .networks()
        .iter()
        .map(|(name, data)| NetworkInterfaceInfo {
            name: name.clone(),
            received: data.received(),
            transmitted: data.transmitted(),
        })
        .collect();

    // --- Device info placeholders ---
    // You can replace these with real detection if you have a crate for device type/model/serial
    let device_type = Some("unknown".into());
    let device_model = Some("unknown".into());
    let serial_number = Some("unknown".into());

    Ok(SystemInfo {
        device_type,
        device_model,
        serial_number,
        disks,
        processes,
        networks,
    })
}
