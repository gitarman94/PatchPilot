mod action;
mod command;
mod device;
mod self_update;
mod patchpilot_updater;
mod system_info;
mod service;
mod logging;

use logging::init_logging;

use std::{fs, path::Path};
use nix::unistd::Uid;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Logger handle to keep alive
lazy_static::lazy_static! {
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
            .unwrap_or_else(|| std::path::PathBuf::from("C:\\PatchPilot"));
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

// Ensure logs directory exists
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
    let info = system_info::SystemInfo::gather_blocking();
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

    let handle = init_logging()?;
    {
        let mut guard = LOGGER_HANDLE.lock().await;
        *guard = Some(handle);
    }

    setup_runtime_environment()?;

    #[cfg(target_os = "linux")]
    ensure_systemd_service()?;

    log::info!("PatchPilot client starting…");
    log_initial_system_info();

    call_unused_modules(); // Ensure Rust sees these functions as used

    #[cfg(unix)]
    {
        if let Err(e) = service::run_unix_service().await {
            log::error!("Service error: {}", e);
            return Err(Box::from(e));
        }
    }

    #[cfg(windows)]
    {
        if let Err(e) = service::run_service(Arc::new(std::sync::atomic::AtomicBool::new(true))).await {
            log::error!("Service error: {}", e);
            return Err(Box::from(e));
        }
    }

    Ok(())
}
