mod system_info;
mod service;

use std::{fs, path::Path};
use crate::service::init_logging;
use nix::unistd::Uid; // root check via nix

/// Return the runtime base directory for the current platform.
fn get_base_dir() -> String {
    #[cfg(target_os = "linux")]
    {
        "/opt/patchpilot_client".to_string()
    }

    #[cfg(target_os = "macos")]
    {
        "/Library/Application Support/patchpilot_client".to_string()
    }

    #[cfg(target_os = "windows")]
    {
        let mut path = dirs::data_local_dir()
            .unwrap_or(std::path::PathBuf::from("C:\\PatchPilot"));
        path.push("PatchPilotClient");
        path.to_string_lossy().into_owned()
    }

    // Fallback for other platforms (shouldn't be hit)
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        "/opt/patchpilot_client".to_string()
    }
}

/// Ensure the logs directory exists and has sensible ownership/permissions.
/// This runs before flexi_logger is initialized.
fn ensure_logs_dir() -> Result<(), Box<dyn std::error::Error>> {
    let base_dir = get_base_dir();
    let logs_dir = format!("{}/logs", base_dir);

    // Create directory if missing
    if !Path::new(&logs_dir).exists() {
        fs::create_dir_all(&logs_dir)?;
    }

    // If running as root, set ownership to patchpilot:patchpilot so the service user can write.
    // Use nix to detect root user.
    #[cfg(target_os = "linux")]
    {
        if Uid::effective().is_root() {
            let _ = std::process::Command::new("chown")
                .arg("-R")
                .arg("patchpilot:patchpilot")
                .arg(&logs_dir)
                .output();
        }
        // Ensure logs directory is world-readable/executable so system user can access it
        let _ = std::process::Command::new("chmod")
            .arg("755")
            .arg(&logs_dir)
            .output();
    }

    Ok(())
}

/// Ensure runtime directories and base ownership exist (root-level setup).
/// This intentionally does NOT create the logs directory (that's handled by ensure_logs_dir).
fn setup_runtime_environment() -> Result<(), Box<dyn std::error::Error>> {
    let base_dir = get_base_dir();
    let server_url_file = format!("{}/server_url.txt", base_dir);

    // Ensure application root directory exists
    if !Path::new(&base_dir).exists() {
        fs::create_dir_all(&base_dir)?;
    }

    // Linux: ensure base ownership is set if running as root (installer case).
    #[cfg(target_os = "linux")]
    {
        if Uid::effective().is_root() {
            let _ = std::process::Command::new("chown")
                .arg("-R")
                .arg("patchpilot:patchpilot")
                .arg(&base_dir)
                .output();
        }
    }

    // Warn (does not block) if server URL configuration is missing
    if !Path::new(&server_url_file).exists() {
        println!(
            "WARNING: Missing server URL configuration file at {}",
            server_url_file
        );
        println!("Create it with the PatchPilot server URL inside (example: http://192.168.1.10:8080).");
    }

    Ok(())
}

/// Ensure the systemd unit file and service user exist (Linux only).
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

    // Ensure the service is enabled
    let status = std::process::Command::new("systemctl")
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

/// Log initial system information to assist debugging at startup.
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

/// Application entry point.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ensure base dirs and config checks are present
    setup_runtime_environment()?;

    // Create logs dir and fix ownership BEFORE starting the logger
    ensure_logs_dir()?;

    // On Linux, ensure systemd unit and service user exist
    #[cfg(target_os = "linux")]
    ensure_systemd_service()?;

    // Initialize logging (flexi_logger expects the log directory to exist)
    if let Err(e) = init_logging() {
        eprintln!("Logging initialization failed: {}", e);
        return Err(Box::<dyn std::error::Error>::from(e));
    }

    log::info!("PatchPilot client starting...");
    log_initial_system_info();

    // Platform-specific service main loop
    #[cfg(unix)]
    {
        if let Err(e) = service::run_unix_service().await {
            log::error!("Service error: {}", e);
            return Err(Box::<dyn std::error::Error>::from(e));
        }
    }

    #[cfg(windows)]
    {
        if let Err(e) = service::run_service().await {
            log::error!("Service error: {}", e);
            return Err(Box::<dyn std::error::Error>::from(e));
        }
    }

    Ok(())
}
