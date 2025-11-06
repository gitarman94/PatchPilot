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
        if let Err(e) = service_main() {
            log::error!("Service failed: {}", e);
        }
    }

    fn service_main() -> Result<()> {
        let mut service = windows_service::service_control_handler::ServiceControlHandler::new()?;
        service.set_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: 0,
        })?;

        while SERVICE_RUNNING.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(5));
        }

        service.set_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: 0,
        })?;
        Ok(())
    }
}

#[cfg(unix)]
mod unix_service {
    use super::*;
    // Similar implementation for Unix, omitted here for brevity.
}

fn main() {
    let result = system_info::get_system_info();
    match result {
        Ok(info) => {
            // Here we send the gathered system info back to the server
            log::info!("Device Info: {:?}", info);
            // You can replace the below line with actual server communication if needed
        }
        Err(e) => {
            eprintln!("Error fetching system info: {}", e);
        }
    }
}
