use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde_json::json;
use std::{fs, thread, time::Duration};
use crate::system_info::{get_system_info, SystemInfo};
use log::{info, error};

// --- Configuration constants ---
const ADOPTION_CHECK_INTERVAL: u64 = 30;  // seconds
const SYSTEM_UPDATE_INTERVAL: u64 = 600;  // seconds (10 minutes)

// Reads the server URL from a local configuration file.
// Uses OS-specific paths for flexibility.
fn read_server_url() -> Result<String> {
    #[cfg(unix)]
    let path = "/opt/patchpilot_client/server_url.txt";

    #[cfg(windows)]
    let path = "C:\\ProgramData\\PatchPilot\\server_url.txt";

    let url = fs::read_to_string(path)
        .with_context(|| format!("Failed to read the server URL from {path}"))?;
    Ok(url.trim().to_string())
}

// Retrieves basic device information (serial, type, model)
fn get_device_info() -> (String, String, String) {
    match get_system_info() {
        Ok(device_info) => {
            let device_id = device_info.serial_number.unwrap_or_else(|| "unknown".into());
            let device_type = device_info.device_type.unwrap_or_else(|| "unknown".into());
            let device_model = device_info.device_model.unwrap_or_else(|| "unknown".into());
            (device_id, device_type, device_model)
        }
        Err(e) => {
            error!("Failed to gather device info: {:?}", e);
            ("unknown".into(), "unknown".into(), "unknown".into())
        }
    }
}

// Checks whether the device has been adopted by the PatchPilot server.
fn check_adoption_status(
    client: &Client,
    server_url: &str,
    device_id: &str,
    device_type: &str,
    device_model: &str,
) -> Result<bool> {
    let resp = client
        .post(format!("{}/api/devices/heartbeat", server_url))
        .json(&json!({
            "device_id": device_id,
            "device_type": device_type,
            "device_model": device_model,
        }))
        .send();

    match resp {
        Ok(resp) if resp.status().is_success() => {
            let status_json: serde_json::Value = resp.json()?;
            Ok(status_json
                .get("adopted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false))
        }
        Ok(resp) => {
            error!(
                "Unexpected HTTP {} from adoption check: {:?}",
                resp.status(),
                resp.text().unwrap_or_default()
            );
            Ok(false)
        }
        Err(e) => {
            error!("Error during adoption check: {:?}", e);
            Ok(false)
        }
    }
}

// Sends detailed system status updates to the server periodically.
fn send_system_update(client: &Client, server_url: &str, device_id: &str) {
    let sys_info = match get_system_info() {
        Ok(info) => info,
        Err(e) => {
            error!("Failed to gather full system info: {:?}", e);
            SystemInfo::default()
        }
    };

    info!("Sending system update for device {}...", device_id);

    let res = client
        .post(format!("{}/api/devices/update_status", server_url))
        .json(&json!({
            "device_id": device_id,
            "status": "active",
            "system_info": sys_info,
        }))
        .send();

    if let Err(e) = res {
        error!("Failed to send system update: {:?}", e);
    }
}

// ---------------------------------------------------------------------------
// --- Unix Service Implementation ---
// ---------------------------------------------------------------------------
#[cfg(unix)]
mod unix_service {
    use super::*;

    pub fn run_unix_service() -> Result<()> {
        info!("Starting PatchPilot Unix service...");

        let client = Client::new();
        let server_url = read_server_url()?;
        let (device_id, device_type, device_model) = get_device_info();

        // --- Adoption Loop ---
        loop {
            info!("Checking device adoption status...");
            match check_adoption_status(&client, &server_url, &device_id, &device_type, &device_model) {
                Ok(true) => {
                    info!("Device adopted successfully. Entering active update mode...");
                    break;
                }
                Ok(false) => {
                    info!(
                        "Device not yet adopted. Retrying in {} seconds...",
                        ADOPTION_CHECK_INTERVAL
                    );
                    thread::sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL));
                }
                Err(e) => {
                    error!("Adoption check failed: {:?}", e);
                    thread::sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL));
                }
            }
        }

        // --- Regular Update Loop ---
        loop {
            send_system_update(&client, &server_url, &device_id);
            thread::sleep(Duration::from_secs(SYSTEM_UPDATE_INTERVAL));
        }
    }
}

// ---------------------------------------------------------------------------
// --- Windows Service Implementation ---
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod windows_service {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use windows_service::{
        define_windows_service, service_dispatcher,
        service_control_handler::{self, ServiceControl, ServiceControlHandlerResult},
        service::{ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType},
    };

    define_windows_service!(ffi_service_main, service_entry);

    static SERVICE_RUNNING: AtomicBool = AtomicBool::new(true);

    pub fn run_service() -> Result<()> {
        info!("Starting PatchPilot Windows service...");
        service_dispatcher::start("PatchPilotService", ffi_service_main)?;
        Ok(())
    }

    fn service_entry(_argc: u32, _argv: *mut *mut u16) {
        if let Err(e) = run() {
            error!("Service error: {:?}", e);
        }
    }

    fn run() -> Result<()> {
        let status_handle = service_control_handler::register(
            "PatchPilotService",
            move |control_event| match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    info!("Service stop signal received.");
                    SERVICE_RUNNING.store(false, Ordering::SeqCst);
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            },
        )?;

        let mut status = ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::from_secs(0),
            process_id: None,
        };
        status_handle.set_service_status(status)?;

        let client = Client::new();
        let server_url = read_server_url()?;
        let (device_id, device_type, device_model) = get_device_info();

        // --- Adoption Loop ---
        while SERVICE_RUNNING.load(Ordering::SeqCst) {
            info!("Checking adoption status...");
            match check_adoption_status(&client, &server_url, &device_id, &device_type, &device_model)
            {
                Ok(true) => {
                    info!("Device adopted successfully. Starting update loop...");
                    break;
                }
                Ok(false) => {
                    thread::sleep(Duration::from_secs(super::ADOPTION_CHECK_INTERVAL));
                }
                Err(e) => {
                    error!("Adoption check error: {:?}", e);
                    thread::sleep(Duration::from_secs(super::ADOPTION_CHECK_INTERVAL));
                }
            }
        }

        // --- Regular Update Loop ---
        while SERVICE_RUNNING.load(Ordering::SeqCst) {
            send_system_update(&client, &server_url, &device_id);
            thread::sleep(Duration::from_secs(super::SYSTEM_UPDATE_INTERVAL));
        }

        info!("Service stopped.");
        status.current_state = ServiceState::Stopped;
        status_handle.set_service_status(status)?;
        Ok(())
    }
}

// Re-export correct function based on platform
#[cfg(unix)]
pub use unix_service::run_unix_service;

#[cfg(windows)]
pub use windows_service::run_service;
