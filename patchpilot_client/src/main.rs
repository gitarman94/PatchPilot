mod system_info;

use anyhow::Result;
use std::{fs, thread, time::Duration};

#[cfg(windows)]
mod windows_service {
    use super::*;
    use lazy_static::lazy_static;
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows_service::{
        define_windows_service, service_dispatcher,
        service_control_handler::{ServiceControl, ServiceControlHandlerResult},
        service::{ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType},
    };

    lazy_static! {
        static ref SERVICE_RUNNING: AtomicBool = AtomicBool::new(true);
    }

    define_windows_service!(ffi_service_main, my_service_main);

    fn read_server_url() -> Result<String> {
        let url = fs::read_to_string("/opt/patchpilot_client/server_url.txt")?;
        Ok(url.trim().to_string())
    }

    pub fn run_service() -> Result<()> {
        log::info!("Starting Windows PatchPilot client service...");
        service_dispatcher::start("PatchPilotClientService", ffi_service_main)?;
        Ok(())
    }

    fn my_service_main(_argc: u32, _argv: *mut *mut u16) {
        if let Err(e) = run_loop() {
            log::error!("Service error: {:?}", e);
        }
    }

    fn run_loop() -> Result<()> {
        let status_handle = windows_service::service_control_handler::register(
            "PatchPilotClientService",
            move |control_event| match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    log::info!("Service stopping...");
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

        super::run_client_loop(|| SERVICE_RUNNING.load(Ordering::SeqCst), read_server_url)?;

        status.current_state = ServiceState::Stopped;
        status_handle.set_service_status(status)?;
        Ok(())
    }
}

#[cfg(unix)]
mod unix_service {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    pub fn run_unix_service() -> Result<()> {
        log::info!("Starting Unix PatchPilot client service...");
        static SERVICE_RUNNING: AtomicBool = AtomicBool::new(true);

        run_client_loop(|| SERVICE_RUNNING.load(Ordering::SeqCst), || {
            let url = fs::read_to_string("/opt/patchpilot_client/server_url.txt")?;
            Ok(url.trim().to_string())
        })
    }
}

// Shared client loop for both platforms
fn run_client_loop<F>(is_running: F, read_server_url: impl Fn() -> Result<String>) -> Result<()>
where
    F: Fn() -> bool,
{
    use reqwest::blocking::Client;
    use serde_json::json;

    let client = Client::new();
    let server_url = read_server_url()?;

    // Device ID & system info
    let device_info_json = system_info::get_system_info()?;
    let device_id = device_info_json.get("system_info")
        .and_then(|si| si.get("serial_number"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let device_type_val = device_info_json.get("system_info")
        .and_then(|si| si.get("device_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let device_model_val = device_info_json.get("system_info")
        .and_then(|si| si.get("device_model"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // --- Registration ---
    loop {
        log::info!("Registering device {}...", device_id);
        match client.post(format!("{}/api/devices/{}", server_url, device_id))
            .json(&device_info_json)
            .send() {
            Ok(r) if r.status().is_success() => {
                log::info!("Device registered successfully");
                break;
            }
            Ok(r) => log::error!("Registration failed, status: {}", r.status()),
            Err(e) => log::error!("Registration error: {:?}", e),
        }
        thread::sleep(Duration::from_secs(30));
    }

    // --- Heartbeat loop ---
    while is_running() {
        let sys_info_update = system_info::get_system_info().unwrap_or(json!({}));
        let network_info = system_info::get_network_info().unwrap_or(json!({}));

        log::info!("Sending heartbeat for device {}", device_id);

        let _ = client.post(format!("{}/api/devices/heartbeat", server_url))
            .json(&json!({
                "device_id": device_id,
                "device_type": device_type_val,
                "device_model": device_model_val,
                "system_info": sys_info_update["system_info"],
                "network_info": network_info
            }))
            .send();

        thread::sleep(Duration::from_secs(600));
    }

    Ok(())
}

fn main() {
    env_logger::init();

    #[cfg(windows)]
    windows_service::run_service().unwrap();

    #[cfg(unix)]
    unix_service::run_unix_service().unwrap();
}
