use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde_json::json;
use std::{fs, thread, time::Duration};
use crate::system_info::{get_system_info, SystemInfo};
use log::{info, error};

// --- Configuration constants ---
const ADOPTION_CHECK_INTERVAL: u64 = 30;   // seconds
const SYSTEM_UPDATE_INTERVAL: u64 = 600;  // seconds

// Reads the server URL from a file
fn read_server_url() -> Result<String> {
    let url = fs::read_to_string("/opt/patchpilot_client/server_url.txt")
        .context("Failed to read the server URL from file")?;
    Ok(url.trim().to_string())
}

// Retrieves device-specific information (ID, type, model)
fn get_device_info() -> (String, String, String) {
    let device_info = get_system_info().unwrap_or_default();
    let device_id = device_info
        .serial_number
        .unwrap_or_else(|| "unknown".to_string());
    let device_type = device_info.device_type.unwrap_or_else(|| "unknown".to_string());
    let device_model = device_info.device_model.unwrap_or_else(|| "unknown".to_string());
    (device_id, device_type, device_model)
}

// Checks adoption status with server
fn check_adoption_status(
    client: &Client,
    server_url: &str,
    device_id: &str,
    device_type: &str,
    device_model: &str,
) -> Result<bool> {
    match client
        .post(format!("{}/api/devices/heartbeat", server_url))
        .json(&json!({
            "device_id": device_id,
            "device_type": device_type,
            "device_model": device_model,
        }))
        .send()
    {
        Ok(resp) if resp.status().is_success() => {
            let status_json: serde_json::Value = resp.json()?;
            Ok(status_json.get("adopted").and_then(|v| v.as_bool()).unwrap_or(false))
        }
        Ok(resp) => {
            error!("Unexpected response while checking adoption status: {:?}", resp.status());
            Ok(false)
        }
        Err(e) => {
            error!("Error checking adoption status: {:?}", e);
            Ok(false)
        }
    }
}

// Sends a system update to the server
fn send_system_update(client: &Client, server_url: &str, device_id: &str) {
    let sys_info = match get_system_info() {
        Ok(info) => info,
        Err(e) => {
            error!("Failed to gather system info: {:?}", e);
            SystemInfo::default()
        }
    };

    info!("Sending system update: {:?}", sys_info);

    if let Err(e) = client
        .post(format!("{}/api/devices/update_status", server_url))
        .json(&json!({
            "device_id": device_id,
            "status": "active",
            "system_info": sys_info,
        }))
        .send()
    {
        error!("Failed to send system update: {:?}", e);
    }
}

// --- Unix Service ---
#[cfg(unix)]
mod unix_service {
    use super::*;
    pub fn run_unix_service() -> Result<()> {
        info!("Starting Unix PatchPilot service...");

        let client = Client::new();
        let server_url = read_server_url()?;
        let (device_id, device_type, device_model) = get_device_info();

        // Adoption check loop
        loop {
            info!("Checking adoption status...");
            if check_adoption_status(&client, &server_url, &device_id, &device_type, &device_model)? {
                info!("Device adopted successfully. Starting regular updates...");
                break;
            }
            info!("Device not yet adopted. Retrying in {} seconds...", ADOPTION_CHECK_INTERVAL);
            thread::sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL));
        }

        // Regular system update loop
        loop {
            send_system_update(&client, &server_url, &device_id);
            thread::sleep(Duration::from_secs(SYSTEM_UPDATE_INTERVAL));
        }
    }
}

// --- Windows Service ---
#[cfg(windows)]
mod windows_service {
    use super::*;
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    use windows_service::{
        define_windows_service, service_dispatcher,
        service_control_handler::{self, ServiceControl, ServiceControlHandlerResult},
        service::{ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType},
    };

    define_windows_service!(ffi_service_main, my_service_main);

    static SERVICE_RUNNING: AtomicBool = AtomicBool::new(true);

    pub fn run_service() -> Result<()> {
        info!("Starting Windows PatchPilot service...");
        service_dispatcher::start("RustPatchDeviceService", ffi_service_main)?;
        Ok(())
    }

    fn my_service_main(_argc: u32, _argv: *mut *mut u16) {
        if let Err(e) = run() {
            error!("Service encountered an error: {:?}", e);
        }
    }

    fn run() -> Result<()> {
        let status_handle = service_control_handler::register(
            "RustPatchDeviceService",
            move |control_event| match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    info!("Service stopping...");
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

        // Adoption loop
        while SERVICE_RUNNING.load(Ordering::SeqCst) {
            info!("Checking adoption status...");
            if check_adoption_status(&client, &server_url, &device_id, &device_type, &device_model)? {
                info!("Device adopted successfully. Starting updates...");
                break;
            }
            thread::sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL));
        }

        // Regular system update loop
        while SERVICE_RUNNING.load(Ordering::SeqCst) {
            send_system_update(&client, &server_url, &device_id);
            thread::sleep(Duration::from_secs(SYSTEM_UPDATE_INTERVAL));
        }

        info!("Service stopped.");
        status.current_state = ServiceState::Stopped;
        status_handle.set_service_status(status)?;
        Ok(())
    }
}

#[cfg(unix)]
pub use unix_service::run_unix_service;

#[cfg(windows)]
pub use windows_service::run_service;
