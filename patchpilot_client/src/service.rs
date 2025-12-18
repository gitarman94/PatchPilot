use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use anyhow::Result;
use reqwest::Client;
use tokio::time::sleep;
use tokio::signal;

use crate::action::start_command_polling;
use crate::device::run_adoption_and_update_loop;
use crate::system_info::{get_system_info_refresh_secs, read_server_url, SystemInfoService};

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

    let running_flag = Arc::new(AtomicBool::new(true));

    // Graceful shutdown on CTRL-C
    {
        let flag = running_flag.clone();
        tokio::spawn(async move {
            let _ = signal::ctrl_c().await;
            println!("CTRL-C received, shutting down...");
            flag.store(false, Ordering::SeqCst);
        });
    }

    // Run adoption/update loop
    let device_id =
        run_adoption_and_update_loop(&client, &server_url, Some(running_flag.clone())).await?;

    // Start async command polling
    start_command_polling(
        client,
        server_url,
        device_id,
        Some(running_flag.clone()),
    )
    .await?;

    Ok(())
}

/// Windows service entrypoint (fully async)
#[cfg(windows)]
pub async fn run_service() -> Result<()> {
    use windows_service::{
        service::{ServiceControl, ServiceControlHandlerResult},
        service_control_handler,
    };

    let client = Client::new();
    let server_url = read_server_url().await?;
    let running_flag = Arc::new(AtomicBool::new(true));

    // Register service control handler
    {
        let flag_for_handler = running_flag.clone();
        service_control_handler::register("PatchPilot", move |control| {
            match control {
                ServiceControl::Stop => {
                    println!("Service stop requested, shutting down...");
                    flag_for_handler.store(false, Ordering::SeqCst);
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        })?;
    }

    // Run adoption/update loop
    let device_id =
        run_adoption_and_update_loop(&client, &server_url, Some(running_flag.clone())).await?;

    // Start async command polling
    start_command_polling(
        client,
        server_url,
        device_id,
        Some(running_flag.clone()),
    )
    .await?;

    Ok(())
}

/// Periodic system info collection loop
pub async fn system_info_loop(
    service: Arc<SystemInfoService>,
    running: Arc<AtomicBool>,
    client: Client,
    server_url: String,
    device_id: String,
) {
    let interval = Duration::from_secs(get_system_info_refresh_secs());

    while running.load(Ordering::SeqCst) {
        match service.get_system_info_async().await {
            Ok(info) => {
                println!("Collected system info: {:?}", info);

                // Send to server
                let url = format!("{}/api/devices/{}/system_info", server_url, device_id);
                let client_clone = client.clone();
                let info_clone = info.clone();

                tokio::spawn(async move {
                    if let Err(e) = client_clone.post(&url).json(&info_clone).send().await {
                        eprintln!("Failed to send system info: {:?}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Failed to collect system info: {:?}", e);
            }
        }

        sleep(interval).await;
    }
}

