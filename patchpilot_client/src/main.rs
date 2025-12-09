mod system_info;
mod service;

use std::{fs, path::Path};
use crate::service::init_logging;
use nix::unistd::Uid;

use lazy_static::lazy_static;
use std::sync::Mutex;

lazy_static! {
    static ref LOGGER_HANDLE: Mutex<Option<flexi_logger::LoggerHandle>> = Mutex::new(None);
}

/// Determine platform-specific application base directory.
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

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        "/opt/patchpilot_client".to_string()
    }
}

/// Ensure runtime directory exists and chown everything once.
fn setup_runtime_environment() -> Result<(), Box<dyn std::error::Error>> {
    let base_dir = get_base_dir();

    if !Path::new(&base_dir).exists() {
        fs::create_dir_all(&base_dir)?;
    }

    // One place for ownership: entire base dir.
    #[cfg(target_os = "linux")]
    {
        if Uid::effective().is_root() {
            let _ = std::process::Command::new("chown")
                .arg("-R")
                .arg("patchpilot:patchpilot")
                .arg(&base_dir)
                .output();

            // Base directory should be readable and enterable by patchpilot user
            let _ = std::process::Command::new("chmod")
                .arg("750")
                .arg(&base_dir)
                .output();
        }
    }

    // Mark missing server_url.txt for later logged warning
    let server_url_file = format!("{}/server_url.txt", base_dir);
    if !Path::new(&server_url_file).exists() {
        std::fs::write(format!("{}/.missing_server_url_flag", base_dir), b"1").ok();
    }

    Ok(())
}

/// Guarantee logs directory exists with correct ownership/permissions.
fn ensure_logs_dir() -> Result<(), Box<dyn std::error::Error>> {
    let base_dir = get_base_dir();
    let logs_dir = format!("{}/logs", base_dir);

    if !Path::new(&logs_dir).exists() {
        fs::create_dir_all(&logs_dir)?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&logs_dir)?.permissions();
        perms.set_mode(0o777);
        fs::set_permissions(&logs_dir, perms)?;
    }

    Ok(())
}

/// Ensure systemd service and service user exist (Linux).
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

    if !Path::new(service_path).exists() {
        let service_contents = r#"[Unit]
Description=PatchPilot Client
After=network.target

[Service]
User=patchpilot
Group=patchpilot
WorkingDirectory=/opt/patchpilot_client
ExecStart=/opt/patchpilot_client/patchpilot_client
Restart=always
Environment=RUST_LOG=info
ReadWritePaths=/opt/patchpilot_client/logs

StandardOutput=null
StandardError=null

[Install]
WantedBy=multi-user.target
"#;

        fs::write(service_path, service_contents)?;
        let _ = std::process::Command::new("systemctl")
            .arg("daemon-reload")
            .output();
    }

    // Enable service if not already enabled
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

/// Log system information at startup.
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
    log::info!("IP Address: {:?}", service::get_ip_address());
    log::info!("Architecture: {}", info.architecture);
    log::info!("Device Type: {:?}", info.device_type);
    log::info!("Device Model: {:?}", info.device_model);
    log::info!("Serial Number: {:?}", info.serial_number);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_runtime_environment()?;
    ensure_logs_dir()?;

    #[cfg(target_os = "linux")]
    ensure_systemd_service()?;

    let handle = init_logging()?; // now returns LoggerHandle
    {
        let mut g = LOGGER_HANDLE.lock().unwrap();
        *g = Some(handle);
    }

    let base_dir = get_base_dir();
    if Path::new(&format!("{}/.missing_server_url_flag", base_dir)).exists() {
        log::error!(
            "Missing server URL configuration at {}/server_url.txt. \
             Create file containing the server URL (e.g. http://192.168.1.10:8080).",
            base_dir
        );
        let _ = fs::remove_file(format!("{}/.missing_server_url_flag", base_dir));
    }

    log::info!("PatchPilot client starting...");
    log_initial_system_info();

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
