use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use tokio::time::{sleep, Duration};

use anyhow::Result;
use reqwest::Client;
use tokio::signal::ctrl_c;

use crate::action::{self, action_loop};
use crate::device::run_adoption_and_update_loop;
use crate::system_info::{self, get_system_info_refresh_secs, read_server_url, SystemInfoService};
use crate::command;

/// Initialize logging for both Unix and Windows
pub fn init_logging() -> anyhow::Result<flexi_logger::LoggerHandle> {
    use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};

    let log_dir: PathBuf = crate::get_base_dir().into();
    let log_dir = log_dir.join("logs");
    std::fs::create_dir_all(&log_dir)?;

    let handle = Logger::try_with_str("info")?
        .log_to_file(
            FileSpec::default()
                .directory(&log_dir)
                .basename("patchpilot_client")
                .suffix("log"),
        )
        .rotate(
            Criterion::Size(5_000_000),
            Naming::Numbers,
            Cleanup::KeepLogFiles(10),
        )
        .duplicate_to_stderr(Duplicate::Info)
        .start()?;

    Ok(handle)
}

/// Common shutdown signal setup
async fn setup_shutdown_signal(running_flag: Arc<AtomicBool>) {
    let flag = running_flag.clone();
    tokio::spawn(async move {
        let _ = ctrl_c().await;
        println!("CTRL-C received, shutting down…");
        flag.store(false, Ordering::SeqCst);
    });
}

/// Unix service entrypoint
#[cfg(any(unix, target_os = "macos"))]
pub async fn run_unix_service() -> Result<()> {
    let client = Client::new();
    let server_url = read_server_url().await?;
    let running_flag = Arc::new(AtomicBool::new(true));

    // Graceful shutdown
    setup_shutdown_signal(running_flag.clone()).await;

    // Device registration and adoption
    let device_id = run_adoption_and_update_loop(&client, &server_url, Some(running_flag.clone())).await?;

    // Start system info loop
    let sys_service = Arc::new(SystemInfoService::default());
    let client_clone = client.clone();
    let srv_clone = server_url.clone();
    let dev_clone = device_id.clone();
    let rf_clone = running_flag.clone();
    let svc_clone = sys_service.clone();
    tokio::spawn(async move {
        crate::service::system_info_loop(svc_clone, rf_clone, client_clone, srv_clone, dev_clone).await;
    });

    // Start action loop
    action_loop(client.clone(), server_url.clone(), device_id.clone(), Some(running_flag.clone())).await?;

    Ok(())
}

/// Windows service entrypoint
#[cfg(windows)]
pub async fn run_service(running_flag: Arc<AtomicBool>) -> Result<()> {
    let client = Client::new();
    let server_url = system_info::read_server_url().await?;

    // Device registration and adoption
    let device_id = run_adoption_and_update_loop(client.clone(), server_url.clone(), running_flag.clone()).await?;

    // Start system info loop
    let sys_service = Arc::new(SystemInfoService::default());
    let client_clone = client.clone();
    let srv_clone = server_url.clone();
    let dev_clone = device_id.clone();
    let rf_clone = running_flag.clone();
    let svc_clone = sys_service.clone();
    tokio::spawn(async move {
        crate::service::system_info_loop(svc_clone, rf_clone, client_clone, srv_clone, dev_clone).await;
    });

    // Start action loop
    action_loop(client.clone(), server_url.clone(), device_id.clone(), Some(running_flag.clone())).await?;

    Ok(())
}

/// Shared periodic system info loop (can be called independently if needed)
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
