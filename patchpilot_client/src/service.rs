#[cfg(windows)]
mod windows_service {
    use anyhow::Result;
    use reqwest::blocking::Client;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::{thread, time::Duration};

    use crate::system_info;

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
        let status_handle = service_control_handler::register("RustPatchDeviceService", move |control_event| {
            match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    log::info!("Service stopping...");
                    SERVICE_RUNNING.lock().unwrap().store(false, Ordering::SeqCst);
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        })?;

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
        let server_url = "http://127.0.0.1:8080"; // Replace with actual server URL

        // Gather dynamic device info once
        let device_id = system_info::get_system_info()?.get("serial_number").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
        let device_type = system_info::get_device_type();
        let device_model = system_info::get_device_model();

        // Heartbeat and adoption check loop
        while SERVICE_RUNNING.lock().unwrap().load(Ordering::SeqCst) {
            log::info!("Checking adoption status for device...");

            let response = client.post(format!("{}/api/devices/heartbeat", server_url))
                .json(&json!({
                    "device_id": device_id,
                    "device_type": device_type,
                    "device_model": device_model,
                }))
                .send();

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let status: serde_json::Value = resp.json()?;
                    if status["adopted"].as_bool() == Some(true) {
                        log::info!("Device approved. Starting regular updates...");
                        break;
                    } else {
                        log::info!("Waiting for approval...");
                    }
                },
                Ok(_) => log::error!("Unexpected response received while checking adoption status."),
                Err(e) => log::error!("Error checking adoption status: {:?}", e),
            }

            thread::sleep(Duration::from_secs(30));
        }

        // Regular system update loop
        while SERVICE_RUNNING.lock().unwrap().load(Ordering::SeqCst) {
            log::info!("Sending system update for device...");

            // Gather dynamic system info
            let sys_info = match system_info::get_system_info() {
                Ok(info) => info,
                Err(e) => {
                    log::error!("Failed to gather system info: {:?}", e);
                    json!({})
                }
            };

            let response = client.post(format!("{}/api/devices/update_status", server_url))
                .json(&json!({
                    "device_id": device_id,
                    "status": "active",
                    "system_info": sys_info,
                }))
                .send();

            if let Err(e) = response {
                log::error!("Failed to send system update: {:?}", e);
            }

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
    use anyhow::Result;
    use reqwest::blocking::Client;
    use serde_json::json;
    use std::{thread, time::Duration};

    use crate::system_info;

    pub fn run_unix_service() -> Result<()> {
        log::info!("Starting Unix service...");

        let client = Client::new();
        let server_url = "http://127.0.0.1:8080"; 

        let system_info_json = system_info::get_system_info()?;
        let device_id = system_info_json.get("serial_number").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
        let device_type = system_info::get_device_type();
        let device_model = system_info::get_device_model();

        loop {
            log::info!("Checking adoption status for device...");

            let response = client.post(format!("{}/api/devices/heartbeat", server_url))
                .json(&json!({
                    "device_id": device_id,
                    "device_type": device_type,
                    "device_model": device_model,
                }))
                .send();

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let status: serde_json::Value = resp.json()?;
                    if status["adopted"].as_bool() == Some(true) {
                        log::info!("Device approved. Starting system updates...");
                        break;
                    } else {
                        log::info!("Waiting for approval...");
                    }
                },
                Ok(_) => log::error!("Unexpected response received while checking adoption status."),
                Err(e) => log::error!("Error checking adoption status: {:?}", e),
            }

            thread::sleep(Duration::from_secs(30));
        }

        loop {
            log::info!("Sending system update for device...");

            let sys_info = match system_info::get_system_info() {
                Ok(info) => info,
                Err(e) => {
                    log::error!("Failed to gather system info: {:?}", e);
                    json!({})
                }
            };

            let response = client.post(format!("{}/api/devices/update_status", server_url))
                .json(&json!({
                    "device_id": device_id,
                    "status": "active",
                    "system_info": sys_info,
                }))
                .send();

            if let Err(e) = response {
                log::error!("Failed to send system update: {:?}", e);
            }

            thread::sleep(Duration::from_secs(600));
        }
    }
}

#[cfg(windows)]
pub use windows_service::run_service;

#[cfg(unix)]
pub use unix_service::run_unix_service;
