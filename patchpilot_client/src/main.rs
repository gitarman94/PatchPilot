mod system_info;
mod service;

use std::{fs, path::Path};
use crate::service::init_logging;

/// Ensures runtime directories and config files exist.
/// All OS-specific differences handled inside this function.
fn setup_runtime_environment() -> Result<(), Box<dyn std::error::Error>> {
    // --- Cross-platform base directory selection ---
    #[cfg(target_os = "linux")]
    let base_dir = "/opt/patchpilot_client";

    #[cfg(target_os = "macos")]
    let base_dir = "/Library/Application Support/patchpilot_client";

    #[cfg(target_os = "windows")]
    let base_dir = {
        let mut path = dirs::data_local_dir()
            .unwrap_or(std::path::PathBuf::from("C:\\PatchPilot"));
        path.push("PatchPilotClient");
        path.to_str().unwrap().into()
    };

    let logs_dir = format!("{}/logs", base_dir);
    let server_url_file = format!("{}/server_url.txt", base_dir);

    // --- Ensure base directory exists ---
    if !Path::new(base_dir).exists() {
        fs::create_dir_all(base_dir)?;
    }

    // --- Ensure logs dir exists ---
    if !Path::new(&logs_dir).exists() {
        fs::create_dir_all(&logs_dir)?;
    }

    // --- Ensure server_url.txt exists ---
    if !Path::new(&server_url_file).exists() {
        fs::write(&server_url_file, "http://0.0.0.0:8080")?;
    }

    // --- Linux-only: fix permissions for patchpilot user ---
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("chown")
            .arg("-R")
            .arg("patchpilot:patchpilot")
            .arg(base_dir)
            .output();
    }

    Ok(())
}

fn log_initial_system_info() {
    use system_info::SystemInfo;

    let mut info = SystemInfo::new();
    info.refresh();

    let (disk_total, disk_free) = info.disk_usage();
    let net = info.network_throughput();

    log::info!("=== Initial System Information ===");
    log::info!("Hostname: {:?}", info.hostname);
    log::info!("OS Name: {:?}", info.os_name);
    log::info!("OS Version: {:?}", info.os_version);
    log::info!("Kernel Version: {:?}", info.kernel_version);
    log::info!("CPU Usage: {:.2}%", info.cpu_usage());
    log::info!(
        "RAM: total {} KB, used {} KB, free {} KB",
        info.ram_total, info.ram_used, info.ram_free
    );
    log::info!("Disk: total {} bytes, free {} bytes", disk_total, disk_free);
    log::info!("Network throughput (initial): {} bytes", net);
    log::info!("IP Address: {:?}", info.ip_address);
    log::info!("Architecture: {}", info.architecture);
    log::info!("Device Type: {:?}", info.device_type);
    log::info!("Device Model: {:?}", info.device_model);
    log::info!("Serial Number: {:?}", info.serial_number);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 🔧 Ensure all directories exist before logging initializes
    setup_runtime_environment()?;

    // Initialize flexi_logger
    if let Err(e) = init_logging() {
        eprintln!("❌ Failed to initialize logging: {e}");
        return Err(Box::<dyn std::error::Error>::from(e));
    }

    log::info!("📌 PatchPilot client starting up...");
    log_initial_system_info();

    // ------------ Platform-specific service start ------------
    #[cfg(unix)]
    {
        if let Err(e) = service::run_unix_service().await {
            log::error!("Unix service error: {e}");
            return Err(Box::<dyn std::error::Error>::from(e));
        }
    }

    #[cfg(windows)]
    {
        if let Err(e) = service::run_service().await {
            log::error!("Windows service error: {e}");
            return Err(Box::<dyn std::error::Error>::from(e));
        }
    }

    Ok(())
}
