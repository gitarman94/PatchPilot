use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use std::path::PathBuf;
use anyhow::Result;
use reqwest::Client;
use crate::device::run_adoption_and_update_loop;
use crate::action::start_command_polling;
use crate::system_info::{SystemInfoService, get_system_info_refresh_secs, read_server_url};
use tokio::time::sleep;

/// Initialize logging for both Unix and Windows
pub fn init_logging() -> anyhow::Result<flexi_logger::LoggerHandle> {
    use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};

    let log_dir: PathBuf = crate::get_base_dir().into();
    let log_dir = log_dir.join("logs");
    std::fs::create_dir_all(&log_dir)?;

    let handle = Logger::try_with_str("info")?
        .log_to_file(
            FileSpec::default()
                .directory(log_dir)
                .basename("patchpilot_client")
                .suffix("log"),
        )
        .rotate(
            Criterion::Size(5_000_000), // 5 MB
            Naming::Numbers,
            Cleanup::KeepLogFiles(10),
        )
        .duplicate_to_stderr(Duplicate::Info)
        .start()?;

    Ok(handle)
}

/// Unix service entrypoint
#[cfg(any(unix, target_os = "macos"))]
pub async fn run_unix_service() -> Result<()> {
    let client = Client::new();
    let server_url = read_server_url().await?;

    // Run adoption/update loop and capture device_id
    let device_id = run_adoption_and_update_loop(&client, &server_url, None).await?;

    // Optional shutdown flag for graceful stop
    let running_flag = Arc::new(AtomicBool::new(true));

    // Start async command polling
    start_command_polling(client, server_url, device_id, Some(running_flag)).await?;

    Ok(())
}

/// Windows service entrypoint (fully async, no futures::block_on)
#[cfg(windows)]
pub async fn run_service() -> Result<()> {
    use windows_service::{
        service::{ServiceControl, ServiceControlHandlerResult},
        service_control_handler,
    };

    let client = Client::new();
    let server_url = read_server_url().await?;
    let running_flag = Arc::new(AtomicBool::new(true));

    // Register service control handler to handle Stop commands
    let flag_for_handler = running_flag.clone();
    let _status = service_control_handler::register("PatchPilot", move |control| {
        match control {
            ServiceControl::Stop => {
                flag_for_handler.store(false, Ordering::SeqCst);
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    })?;

    // Run device adoption
    let device_id = run_adoption_and_update_loop(&client, &server_url, Some(running_flag.clone())).await?;

    // Start polling for commands asynchronously
    start_command_polling(client, server_url, device_id, Some(running_flag.clone())).await?;

    Ok(())
}

/// System info collection loop
pub async fn system_info_loop(service: Arc<SystemInfoService>) {
    let interval = Duration::from_secs(get_system_info_refresh_secs());
    loop {
        match service.get_system_info_async().await {
            Ok(info) => println!("Collected system info: {:?}", info),
            Err(e) => eprintln!("Failed to collect system info: {:?}", e),
        }
        sleep(interval).await;
    }
}
