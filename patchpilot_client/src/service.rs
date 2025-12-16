use anyhow::Result;
use reqwest::Client;
use std::sync::{Arc, atomic::AtomicBool};

pub mod command; // new module for commands, heartbeat, updates
use command::*;

pub fn init_logging() -> anyhow::Result<flexi_logger::LoggerHandle> {
    // ... your existing init_logging function stays as-is ...
}

/// Unix service entrypoint
#[cfg(any(unix, target_os = "macos"))]
pub async fn run_unix_service() -> Result<()> {
    let client = Client::new();
    let server_url = crate::system_info::read_server_url().await?;
    run_adoption_and_update_loop(&client, &server_url, None).await
}

/// Windows service entrypoint
#[cfg(windows)]
pub async fn run_service() -> Result<()> {
    use windows_service::{
        service::{ServiceControl, ServiceControlHandlerResult},
        service_control_handler,
    };
    let running_flag = Arc::new(AtomicBool::new(true));
    let running_flag_clone = running_flag.clone();

    fn service_main(flag: Arc<AtomicBool>) -> Result<()> {
        let client = Client::new();
        let server_url = futures::executor::block_on(crate::system_info::read_server_url())?;
        futures::executor::block_on(run_adoption_and_update_loop(&client, &server_url, Some(flag)))
    }

    let flag_for_handler = running_flag.clone();
    let _status = service_control_handler::register("PatchPilot", move |control| {
        match control {
            ServiceControl::Stop => {
                flag_for_handler.store(false, std::sync::atomic::Ordering::SeqCst);
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    })?;

    service_main(running_flag_clone)
}
