use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde_json::json;
use std::{fs, thread, time::Duration};
use crate::system_info::{get_system_info, SystemInfo};

// Reads the server URL from a file
fn read_server_url() -> Result<String> {
    let url = fs::read_to_string("/opt/patchpilot_client/server_url.txt")
        .context("Failed to read the server URL from file")?;
    Ok(url.trim().to_string())
}

// Retrieves device-specific information (ID, type, and model)
fn get_device_info() -> (String, String, String) {
    let info: SystemInfo = get_system_info().unwrap_or_default();
    let device_id = info.serial_number.unwrap_or_else(|| "unknown".to_string());
    let device_type = info.device_type.unwrap_or_else(|| "unknown".to_string());
    let device_model = info.device_model.unwrap_or_else(|| "unknown".to_string());
    (device_id, device_type, device_model)
}

// Checks the adoption status of the device on the server
fn check_adoption_status(
    client: &Client,
    server_url: &str,
    device_id: &str,
    device_type: &str,
    device_model: &str,
) -> Result<bool> {
    let response = client
        .post(format!("{}/api/devices/heartbeat", server_url))
        .json(&json!({
            "device_id": device_id,
            "device_type": device_type,
            "device_model": device_model,
        }))
        .send();

    match response {
        Ok(resp) if resp.status().is_success() => {
            let status_json: serde_json::Value = resp.json()?;
            Ok(status_json["adopted"].as_bool() == Some(true))
        }
        Ok(_) => {
            log::error!("Unexpected response received while checking adoption status.");
            Ok(false)
        }
        Err(e) => {
            log::error!("Error checking adoption status: {:?}", e);
            Ok(false)
        }
    }
}

// Sends system information update to the server
fn send_system_update(client: &Client, server_url: &str, device_id: &str) {
    let sys_info: SystemInfo = match get_system_info() {
        Ok(info) => info,
        Err(e) => {
            log::error!("Failed to gather system info: {:?}", e);
            SystemInfo::default()
        }
    };

    log::info!("Sending system update: {:?}", sys_info);

    if let Err(e) = client
        .post(format!("{}/api/devices/update_status", server_url))
        .json(&json!({
            "device_id": device_id,
            "status": "active",
            "system_info": sys_info,
        }))
        .send()
    {
        log::error!("Failed to send system update: {:?}", e);
    }
}

#[cfg(windows)]
mod windows_service {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows_service::{
        define_windows_service, service_dispatcher,
        service_control_handler::{self, ServiceControl, ServiceControlHandlerResult},
        service::{ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType},
    };
    use std::thread;
    use std::time::Duration;

    define_windows_service!(ffi_service_main, my_service_main);

    lazy_static::lazy_static! {
        static ref SERVICE_RUNNING: Arc<Mutex<AtomicBool>> = Arc::new(Mutex::new(AtomicBool::new(true)));
    }

    pub fn run_service() -> Result<()> {
        log::info!("Starting Windows service...");
        service_dispatcher::start("RustPatchDeviceService", ffi_service_main)?;
        Ok(())
    }

    fn my_service_main(_argc: u32, _argv: *mut *mut u16) {
        if let Err(e) = run() {
            log::error!("Service error: {:?}", e);
        }
    }

    fn run() -> Result<()> {
        let status_handle = service_control_handler::register(
            "RustPatchDeviceService",
            move |control_event| match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    log::info!("Service stopping...");
                    SERVICE_RUNNING.lock().unwrap().store(false, Ordering::SeqCst);
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

        // Adoption check loop
        while SERVICE_RUNNING.lock().unwrap().load(Ordering::SeqCst) {
            log::info!("Checking adoption status for device...");
            if check_adoption_status(&client, &server_url, &device_id, &device_type, &device_model)? {
                log::info!("Device approved. Starting regular updates...");
                break;
            }
            log::info!("Waiting for approval...");
            thread::sleep(Duration::from_secs(30));
        }

        // Regular system update loop
        while SERVICE_RUNNING.lock().unwrap().load(Ordering::SeqCst) {
            send_system_update(&client, &server_url, &device_id);
            thread::sleep(Duration::from_secs(600));
        }

        log::info!("Service stopping...");
        status.current_state = ServiceState::Stopped;
        status_handle.set_service_status(status)?;
        Ok(())
    }
}

#[cfg(unix)]
mod unix_service {
    use super::*;
    use std::thread;
    use std::time::Duration;

    pub fn run_unix_service() -> Result<()> {
        log::info!("Starting Unix service...");

        let client = Client::new();
        let server_url = read_server_url()?;
        let (device_id, device_type, device_model) = get_device_info();

        // Adoption check loop
        loop {
            log::info!("Checking adoption status for device...");
            if check_adoption_status(&client, &server_url, &device_id, &device_type, &device_model)? {
                log::info!("Device approved. Starting system updates...");
                break;
            }
            log::info!("Waiting for approval...");
            thread::sleep(Duration::from_secs(30));
        }

        // Regular system update loop
        loop {
            send_system_update(&client, &server_url, &device_id);
            thread::sleep(Duration::from_secs(600));
        }
    }
}

#[cfg(windows)]
pub use windows_service::run_service;

#[cfg(unix)]
pub use unix_service::run_unix_service;
