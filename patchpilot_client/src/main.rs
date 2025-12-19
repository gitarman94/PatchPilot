mod action;
mod command;
mod device;
mod remote_cmd;
mod self_update;
mod patchpilot_updater;
mod system_info;
mod service;

use std::{fs, path::Path};
use crate::service::init_logging;
use nix::unistd::Uid;
use lazy_static::lazy_static;
use std::sync::Mutex;

// Will hold the logger handle so it doesn’t get dropped
lazy_static! {
    static ref LOGGER_HANDLE: Mutex<Option<flexi_logger::LoggerHandle>> = Mutex::new(None);
}

// Determine platform-specific application base directory.
fn get_base_dir() -> String {
    #[cfg(target_os = "linux")] {
        "/opt/patchpilot_client".to_string()
    }
    #[cfg(target_os = "macos")] {
        "/Library/Application Support/patchpilot_client".to_string()
    }
    #[cfg(target_os = "windows")] {
        let mut path = dirs::data_local_dir()
            .unwrap_or(std::path::PathBuf::from("C:\\PatchPilot"));
        path.push("PatchPilotClient");
        path.to_string_lossy().into_owned()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))] {
        "/opt/patchpilot_client".to_string()
    }
}

// Ensure base runtime directories and ownership exist
fn setup_runtime_environment() -> Result<(), Box<dyn std::error::Error>> {
    let base_dir = get_base_dir();
    if !Path::new(&base_dir).exists() {
        fs::create_dir_all(&base_dir)?;
    }

    #[cfg(target_os = "linux")] {
        // Make sure patchpilot user owns this if we're installed as root
        if Uid::effective().is_root() {
            let _ = std::process::Command::new("chown")
                .arg("-R")
                .arg("patchpilot:patchpilot")
                .arg(&base_dir)
                .output();
            let _ = std::process::Command::new("chmod")
                .arg("750")
                .arg(&base_dir)
                .output();
        }
    }

    Ok(())
}

// Ensure logs directory exists with correct ownership/permissions.
fn ensure_logs_dir() -> Result<(), Box<dyn std::error::Error>> {
    let base_dir = get_base_dir();
    let logs_dir = format!("{}/logs", base_dir);

    if !Path::new(&logs_dir).exists() {
        fs::create_dir_all(&logs_dir)?;
    }

    #[cfg(target_os = "linux")] {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&logs_dir)?.permissions();
        perms.set_mode(0o770);
        fs::set_permissions(&logs_dir, perms)?;
        let _ = std::process::Command::new("chown")
            .arg("-R")
            .arg("patchpilot:patchpilot")
            .arg(&logs_dir)
            .output();
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn ensure_systemd_service() -> Result<(), Box<dyn std::error::Error>> {
    let service_path = "/etc/systemd/system/patchpilot_client.service";

    // Ensure patchpilot system user exists
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

    // Write out service file if missing
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
StandardOutput=append:/opt/patchpilot_client/logs/patchpilot_current.log
StandardError=append:/opt/patchpilot_client/logs/patchpilot_current.log

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

fn log_initial_system_info() {
    use system_info::SystemInfo;
    let info = SystemInfo::gather_blocking();
    log::info!("Initial system information:");
    log::info!("Hostname: {:?}", info.hostname);
    log::info!("OS Name: {:?}", info.os_name);
    log::info!("Architecture: {:?}", info.architecture);
    log::info!("CPU Usage: {:.2}%", info.cpu_usage);
    log::info!("RAM: total {} KB, used {} KB", info.ram_total, info.ram_used);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ensure_logs_dir()?;

    // Initialize file logging
    let handle = init_logging()?;
    {
        let mut guard = LOGGER_HANDLE.lock().unwrap();
        *guard = Some(handle);
    }

    setup_runtime_environment()?;

    #[cfg(target_os = "linux")]
    ensure_systemd_service()?;

    log::info!("PatchPilot client starting…");
    log_initial_system_info();

    #[cfg(unix)]
    {
        if let Err(e) = service::run_unix_service().await {
            log::error!("Service error: {}", e);
            return Err(Box::from(e));
        }
    }

    #[cfg(windows)]
    {
        if let Err(e) = service::run_service().await {
            log::error!("Service error: {}", e);
            return Err(Box::from(e));
        }
    }

    Ok(())
}
