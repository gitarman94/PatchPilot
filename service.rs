#[cfg(windows)]
mod windows_service {
    use anyhow::Result;
    use windows_service::{
        define_windows_service,
        service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceStatusHandle, ServiceType,},
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
    };
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex as StdMutex;

    use crate::system_info::{get_system_info, get_missing_windows_updates};
    use reqwest::blocking::Client;
    use serde_json::json;

    define_windows_service!(ffi_service_main, my_service_main);

    lazy_static::lazy_static! {
        static ref SERVICE_RUNNING: StdMutex<AtomicBool> = StdMutex::new(AtomicBool::new(true));
    }

    pub fn run_service() -> Result<()> {
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

        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("Failed to create HTTP client");

        // Update these paths as needed
        let server_url = std::fs::read_to_string(r"C:\ProgramData\RustPatchClient\server_url.txt")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
            .trim()
            .to_string();

        let client_id = std::fs::read_to_string(r"C:\ProgramData\RustPatchClient\client_id.txt")
            .unwrap_or_else(|_| "unknown_client".to_string())
            .trim()
            .to_string();

        while SERVICE_RUNNING.lock().unwrap().load(Ordering::SeqCst) {
            let sys_info = get_system_info().unwrap_or_else(|_| "Failed to get system info".to_string());
            let missing_updates = get_missing_windows_updates().unwrap_or_else(|_| vec![]);

            let report_url = format!("{}/api/devices/{}/update_status", server_url, client_id);
            let payload = json!({
                "system_info": sys_info,
                "missing_updates": missing_updates,
            });

            if let Err(e) = client.post(&report_url).json(&payload).send() {
                log::error!("Failed to send update status: {:?}", e);
            }

            thread::sleep(Duration::from_secs(600));
        }

        status.current_state = ServiceState::Stopped;
        status_handle.set_service_status(status)?;

        Ok(())
    }
}

#[cfg(unix)]
mod unix_service {
    use anyhow::Result;
    use std::{fs, thread, time::Duration};
    use reqwest::blocking::Client;
    use serde_json::json;
    use crate::system_info::get_system_info;

    pub fn run_unix_service() -> Result<()> {
        // Paths for Linux, adjust as needed
        let config_dir = "/etc/patchpilot_client";
        let server_url_path = format!("{}/server_url.txt", config_dir);
        let client_id_path = format!("{}/client_id.txt", config_dir);

        let server_url = fs::read_to_string(&server_url_path)
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
            .trim()
            .to_string();

        let client_id = fs::read_to_string(&client_id_path)
            .unwrap_or_else(|_| "unknown_client".to_string())
            .trim()
            .to_string();

        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("Failed to create HTTP client");

        // Simple run loop (can be replaced with systemd timers)
        loop {
            let sys_info = get_system_info().unwrap_or_else(|_| "Failed to get system info".to_string());

            // Linux: no Windows updates, so empty vector
            let missing_updates: Vec<String> = Vec::new();

            let report_url = format!("{}/api/devices/{}/update_status", server_url, client_id);
            let payload = json!({
                "system_info": sys_info,
                "missing_updates": missing_updates,
            });

            if let Err(e) = client.post(&report_url).json(&payload).send() {
                log::error!("Failed to send update status: {:?}", e);
            }

            thread::sleep(Duration::from_secs(600));
        }
    }
}

#[cfg(windows)]
pub use windows_service::run_service;

#[cfg(unix)]
pub use unix_service::run_unix_service;
