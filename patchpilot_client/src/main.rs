mod system_info;
mod service;

use std::{fs, path::Path};
use crate::service::init_logging;

/// Systemd Setup (Linux Only)
/// Ensures the PatchPilot systemd service exists, is enabled,
/// and the dedicated service user is present.
#[cfg(target_os = "linux")]
fn ensure_systemd_service() -> Result<(), Box<dyn std::error::Error>> {
    let service_path = "/etc/systemd/system/patchpilot_client.service";

    // Ensure service user exists
    let _ = std::process::Command::new("id")
        .arg("patchpilot")
        .output()
        .map(|output| {
            if !output.status.success() {
                let _ = std::process::Command::new("useradd")
                    .arg("-r")
                    .arg("-s")
                    .arg("/usr/sbin/nologin")
                    .arg("patchpilot")
                    .output();
            }
        });

    // Create service unit file if missing
    if !Path::new(service_path).exists() {
        let service_contents = r#"[Unit]
Description=PatchPilot Client
After=network.target

[Service]
User=patchpilot
ExecStart=/opt/patchpilot_client/patchpilot_client
Restart=always

[Install]
WantedBy=multi-user.target
"#;

        fs::write(service_path, service_contents)?;

        let _ = std::process::Command::new("systemctl")
            .arg("daemon-reload")
            .output();
    }

    // Enable service
    let status = std::process::Command::Command::new("systemctl")
        .arg("is-enabled")
        .arg("patchpilot_client.service")
        .output();

    if let Ok(out) = status {
        if !out.status.success() {
            let _ = std::process::Command::new("systemctl")
                .arg("enable")
                .arg("patchpilot_client.service")
                .output();
        }
    }

    Ok(())
}

/// Runtime Environment Setup
/// Creates directories, enforces permissions, and validates
/// expected configuration files.
fn setup_runtime_environment() -> Result<(), Box<dyn std::error::Error>> {
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

    // Ensure application directory structure
    if !Path::new(base_dir).exists() {
        fs::create_dir_all(base_dir)?;
    }
    if !Path::new(&logs_dir).exists() {
        fs::create_dir_all(&logs_dir)?;
    }

    // Linux-specific permission enforcement
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("chown")
            .arg("-R")
            .arg("patchpilot:patchpilot")
            .arg(base_dir)
            .output();

        let _ = std::process::Command::new("chmod")
            .arg("755")
            .arg(&logs_dir)
            .output();
    }

    // Warn (not error) if missing configuration
    if !Path::new(&server_url_file).exists() {
        println!(
            "WARNING: Missing server URL configuration file at {}",
            server_url_file
        );
        println!("Create it with the PatchPilot server URL (e.g. http://192.168.1.10:8080).");
    }

    Ok(())
}

/// Initial System Information Logging
fn log_initial_system_info() {
    use system_info::SystemInfo;

    let mut info = SystemInfo::new();
    info.refresh();

    let (disk_total, disk_free) = info.disk_usage();
    let net = info.network_throughput();

    log::info!("Initial system information:");
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
    log::info!("Initial network throughput: {} bytes", net);
    log::info!("IP Address: {:?}", info.ip_address);
    log::info!("Architecture: {}", info.architecture);
    log::info!("Device Type: {:?}", info.device_type);
    log::info!("Device Model: {:?}", info.device_model);
    log::info!("Serial Number: {:?}", info.serial_number);
}

/// Application Entry Point
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_runtime_environment()?;

    #[cfg(target_os = "linux")]
    ensure_systemd_service()?;

    if let Err(e) = init_logging() {
        eprintln!("Logging initialization failed: {}", e);
        return Err(Box::<dyn std::error::Error>::from(e));
    }

    log::info!("PatchPilot client starting...");
    log_initial_system_info();

    #[cfg(unix)]
    {
        if let Err(e) = service::run_unix_service().await {
            log::error!("Service error: {}", e.to_string());
            return Err(Box::<dyn std::error::Error>::from(e));
        }
    }

    #[cfg(windows)]
    {
        if let Err(e) = service::run_service().await {
            log::error!("Service error: {}", e.to_string());
            return Err(Box::<dyn std::error::Error>::from(e));
        }
    }

    Ok(())
}
