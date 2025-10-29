#[cfg(windows)]
mod windows_service {
    use anyhow::Result;
    use reqwest::blocking::Client;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::{thread, time::Duration};

    define_windows_service!(ffi_service_main, my_service_main);

    lazy_static::lazy_static! {
        static ref SERVICE_RUNNING: Arc<Mutex<AtomicBool>> = Arc::new(Mutex::new(AtomicBool::new(true)));
    }

    pub fn run_service() -> Result<()> {
        log::info!("Starting Windows service...");
        service_dispatcher::start("RustPatchClientService", ffi_service_main)?;
        Ok(())
    }

    fn my_service_main(_argc: u32, _argv: *mut *mut u16) {
        if let Err(e) = run() {
            log::error!("Service error: {:?}", e);
        }
    }

    fn run() -> Result<()> {
        let status_handle = service_control_handler::register("RustPatchClientService", move |control_event| {
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
        let client_id = "unique-client-id"; // Replace with actual client ID

        // Heartbeat and adoption check loop
        while SERVICE_RUNNING.lock().unwrap().load(Ordering::SeqCst) {
            log::info!("Checking adoption status...");

            let response = client.post(format!("{}/api/devices/heartbeat", server_url))
                .json(&json!({ "client_id": client_id }))
                .send();

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let status: serde_json::Value = resp.json()?;
                    if status["adopted"].as_bool() == Some(true) {
                        log::info!("Client approved. Starting regular updates...");
                        break; // Proceed to regular update mode
                    } else {
                        log::info!("Waiting for approval...");
                    }
                },
                Ok(_) => log::error!("Unexpected response received while checking adoption status."),
                Err(e) => log::error!("Error checking adoption status: {:?}", e),
            }

            // Sleep before the next heartbeat check
            thread::sleep(Duration::from_secs(30)); // Heartbeat interval
        }

        // Regular system update loop
        while SERVICE_RUNNING.lock().unwrap().load(Ordering::SeqCst) {
            log::info!("Sending system update...");

            let sys_info = "System info goes here"; // Gather and format system info here
            let response = client.post(format!("{}/api/devices/update_status", server_url))
                .json(&json!( {
                    "client_id": client_id,
                    "status": "active", // Update status
                    "system_info": sys_info,
                }))
                .send();

            if let Err(e) = response {
                log::error!("Failed to send system update: {:?}", e);
            }

            // Sleep before the next system status update
            thread::sleep(Duration::from_secs(600)); // Regular update interval
        }

        // Stop the service
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

    pub fn run_unix_service() -> Result<()> {
        log::info!("Starting Unix service...");

        let client = Client::new();
        let server_url = "http://127.0.0.1:8080"; // Replace with actual server URL
        let client_id = "unique-client-id"; // Replace with actual client ID

        // Heartbeat and adoption check loop
        loop {
            log::info!("Checking adoption status...");

            let response = client.post(format!("{}/api/devices/heartbeat", server_url))
                .json(&json!({ "client_id": client_id }))
                .send();

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let status: serde_json::Value = resp.json()?;
                    if status["adopted"].as_bool() == Some(true) {
                        log::info!("Client approved. Starting system updates...");
                        break; // Proceed to regular update mode
                    } else {
                        log::info!("Waiting for approval...");
                    }
                },
                Ok(_) => log::error!("Unexpected response received while checking adoption status."),
                Err(e) => log::error!("Error checking adoption status: {:?}", e),
            }

            thread::sleep(Duration::from_secs(30)); // Heartbeat interval
        }

        // Regular system update loop
        loop {
            log::info!("Sending system update...");

            let sys_info = "System info goes here"; // Gather and format system info here
            let response = client.post(format!("{}/api/devices/update_status", server_url))
                .json(&json!( {
                    "client_id": client_id,
                    "status": "active", // Update status
                    "system_info": sys_info,
                }))
                .send();

            if let Err(e) = response {
                log::error!("Failed to send system update: {:?}", e);
            }

            thread::sleep(Duration::from_secs(600)); // Regular update interval
        }

        Ok(())
    }
}

#[cfg(windows)]
pub use windows_service::run_service;

#[cfg(unix)]
pub use unix_service::run_unix_service;
