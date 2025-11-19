use local_ip_address::local_ip;
use serde::Serialize;
use sysinfo::{System, SystemExt, CpuExt};

/// FullSystemInfo for sending to server (serialized)
#[derive(Serialize, Default)]
pub struct FullSystemInfo {
    pub hostname: Option<String>,
    pub ip_address: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,

    pub cpu_brand: Option<String>,
    pub cpu_count: Option<usize>,
    pub architecture: String,

    pub device_type: Option<String>,
    pub device_model: Option<String>,
    pub serial_number: Option<String>,

    pub total_memory: u64,
    pub used_memory: u64,
    pub free_memory: u64,
}

/// Returns FullSystemInfo for API reporting
pub fn get_system_info() -> anyhow::Result<FullSystemInfo> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let hostname = sys.host_name();
    let ip_address = local_ip().ok().map(|ip| ip.to_string());
    let os_name = sys.name();
    let os_version = sys.os_version();
    let kernel_version = sys.kernel_version();

    let cpu_brand = sys.cpus().get(0).map(|c| c.brand().to_string());
    let cpu_count = Some(sys.cpus().len());
    let architecture = std::env::consts::ARCH.to_string();

    // Placeholder device info; can be replaced with real detection
    let device_type = Some("unknown".into());
    let device_model = Some("unknown".into());
    let serial_number = Some("undefined".into());

    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let free_memory = total_memory.saturating_sub(used_memory);

    Ok(FullSystemInfo {
        hostname,
        ip_address,
        os_name,
        os_version,
        kernel_version,
        cpu_brand,
        cpu_count,
        architecture,
        device_type,
        device_model,
        serial_number,
        total_memory,
        used_memory,
        free_memory,
    })
}
