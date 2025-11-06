mod system_info;

use anyhow::Result;
use log::{error, info};
use std::{thread, time::Duration};
use serde_json::to_string_pretty;

#[cfg(windows)]
mod windows_service {
    use super::*;
    use lazy_static::lazy_static;
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows_service::{
        define_windows_service, service_dispatcher,
        service_control_handler::{self, ServiceControl, ServiceControlHandlerResult},
        service::{ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType},
    };

    lazy_static! {
        static ref SERVICE_RUNNING: AtomicBool = AtomicBool::new(true);
    }

    define_windows_service!(ffi_service_main, my_service_main);

    pub fn run_service() -> Result<()> {
        info!("Starting Windows PatchPilot client service...");
        service_dispatcher::start("PatchPilotClientService", ffi_service_main)?;
        Ok(())
    }

    fn my_service_main(_argc: u32, _argv: *mut *mut u16) {
        if let Err(e) = service_main() {
            error!("Service failed: {}", e);
        }
    }

    fn service_main() -> Result<()> {
        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop => {
                    SERVICE_RUNNING.store(false, Ordering::SeqCst);
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle =
            service_control_handler::register("PatchPilotClientService", event_handler)?;

        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: 0,
            process_id: None,
        })?;

        while SERVICE_RUNNING.load(Ordering::SeqCst) {
            match crate::system_info::get_system_info() {
                Ok(info) => info!("System info: {}", to_string_pretty(&info).unwrap()),
                Err(e) => error!("Error gathering system info: {}", e),
            }
            thread::sleep(Duration::from_secs(10));
        }

        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: 0,
            process_id: None,
        })?;

        Ok(())
    }
}

#[cfg(unix)]
mod unix_service {
    use super::*;
    use std::thread;
    use std::time::Duration;

    pub fn run_service() -> Result<()> {
        info!("Starting Unix PatchPilot client daemon...");

        loop {
            match crate::system_info::get_system_info() {
                Ok(info) => info!("System info: {}", to_string_pretty(&info).unwrap()),
                Err(e) => error!("Error gathering system info: {}", e),
            }

            thread::sleep(Duration::from_secs(10));
        }
    }
}

fn main() {
    env_logger::init();

    #[cfg(windows)]
    if let Err(e) = windows_service::run_service() {
        error!("Windows service failed: {}", e);
    }

    #[cfg(unix)]
    if let Err(e) = unix_service::run_service() {
        error!("Unix service failed: {}", e);
    }

    // Fallback CLI run
    if let Ok(info) = system_info::get_system_info() {
        println!("Device Info:\n{}", to_string_pretty(&info).unwrap());
    } else {
        eprintln!("Error fetching system info");
    }
}
