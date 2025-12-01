use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use std::{fs, time::Duration, path::PathBuf};
use crate::system_info::{SystemInfo, get_system_info};
use local_ip_address::local_ip;
use tokio::time::sleep;
use std::sync::atomic::{AtomicBool, Ordering};
use simplelog::{Config, WriteLogger, LevelFilter};

const ADOPTION_CHECK_INTERVAL: u64 = 10;
const SYSTEM_UPDATE_INTERVAL: u64 = 600;

#[cfg(any(unix, target_os = "macos"))]
const DEVICE_ID_FILE: &str = "/opt/patchpilot_client/device_id.txt";
#[cfg(windows)]
const DEVICE_ID_FILE: &str = "C:\\ProgramData\\PatchPilot\\device_id.txt";

#[cfg(any(unix, target_os = "macos"))]
const SERVER_URL_FILE: &str = "/opt/patchpilot_client/server_url.txt";
#[cfg(windows)]
const SERVER_URL_FILE: &str = "C:\\ProgramData\\PatchPilot\\server_url.txt";

fn init_logging() -> Result<()> {
    let mut log_path = dirs::home_dir().unwrap_or(PathBuf::from("."));
    log_path.push("patchpilot_client/logs");

    fs::create_dir_all(&log_path)
        .context("Failed creating log directory")?;

    log_path.push("service.log");

    WriteLogger::init(
        LevelFilter::Info,
        Config::default(),
        fs::File::create(log_path).context("Failed creating log file")?,
    )
    .context("Failed initializing logger")
}

async fn read_server_url() -> Result<String> {
    let url = fs::read_to_string(SERVER_URL_FILE)
        .with_context(|| format!("Failed to read {}", SERVER_URL_FILE))?;
    Ok(url.trim().to_string())
}

fn get_ip_address() -> String {
    local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "0.0.0.0".into())
}

fn get_local_device_id() -> Option<String> {
    fs::read_to_string(DEVICE_ID_FILE).ok().map(|s| s.trim().to_string())
}

fn write_local_device_id(device_id: &str) -> Result<()> {
    fs::write(DEVICE_ID_FILE, device_id).context("Failed to write local device_id file")
}

fn get_device_info_basic() -> (String, String) {
    match get_system_info() {
        Ok(info) => {
            let device_type = info.device_type.unwrap_or_else(|| "unknown".into());
            let device_model = info.device_model.unwrap_or_else(|| "unknown".into());
            (device_type, device_model)
        }
        Err(_) => ("unknown".into(), "unknown".into()),
    }
}

async fn send_system_update(client: &Client, server_url: &str, device_id: &str) {
    let mut sys_info = get_system_info().unwrap_or_else(|_| SystemInfo::new());
    sys_info.refresh();

    let _ = client
        .post(format!("{}/api/devices/{}", server_url, device_id))
        .json(&json!({
            "device_id": device_id,
            "status": "active",
            "system_info": sys_info,
            "ip_address": get_ip_address(),
        }))
        .send()
        .await;
}

async fn send_heartbeat(
    client: &Client,
    server_url: &str,
    device_id: &str,
    device_type: &str,
    device_model: &str
) -> bool {
    let resp = client
        .post(format!("{}/api/devices/heartbeat", server_url))
        .json(&json!({
            "device_id": device_id,
            "device_type": device_type,
            "device_model": device_model,
            "ip_address": get_ip_address(),
            "network_interfaces": "eth0,wlan0"
        }))
        .send()
        .await;

    if let Ok(r) = resp {
        if let Ok(v) = r.json::<serde_json::Value>().await {
            return v.get("adopted").and_then(|x| x.as_bool()).unwrap_or(false);
        }
    }
    false
}

async fn run_adoption_and_update_loop(
    client: &Client,
    server_url: &str,
    running_flag: Option<&AtomicBool>
) -> Result<()> {

    let mut device_id = get_local_device_id();
    let (device_type, device_model) = get_device_info_basic();

    if device_id.is_none() {
        log::info!("No device_id found locally. Waiting for server adoption...");

        loop {
            let adopted = send_heartbeat(
                client, server_url,
                "", &device_type, &device_model
            ).await;

            if adopted {
                log::info!("Server assigned a device_id!");

                let resp = client
                    .get(format!("{}/api/devices/assign", server_url))
                    .send()
                    .await?
                    .json::<serde_json::Value>()
                    .await?;

                if let Some(id) = resp.get("device_id").and_then(|v| v.as_str()) {
                    device_id = Some(id.to_string());
                    write_local_device_id(id)?;
                    break;
                }
            }

            sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL)).await;
        }
    }

    let device_id = device_id.unwrap();
    log::info!("Starting heartbeat loop for device {}", device_id);

    loop {
        let adopted = send_heartbeat(
            client, server_url,
            &device_id, &device_type, &device_model
        ).await;

        if adopted {
            log::info!("Device {} approved by server", device_id);
            write_local_device_id(&device_id).ok();
            break;
        }

        sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL)).await;
    }

    log::info!("Entering system update loop for device {}", device_id);

    loop {
        if let Some(flag) = running_flag {
            if !flag.load(Ordering::SeqCst) {
                log::info!("Stopping update loop due to service stop");
                return Ok(());
            }
        }

        send_system_update(client, server_url, &device_id).await;
        sleep(Duration::from_secs(SYSTEM_UPDATE_INTERVAL)).await;
    }
}

#[cfg(any(unix, target_os = "macos"))]
pub async fn run_unix_service() -> Result<()> {
    init_logging()?;

    log::info!("Starting PatchPilot Unix/macOS service...");

    let client = Client::new();
    let server_url = read_server_url().await?;

    run_adoption_and_update_loop(&client, &server_url, None).await
}

#[cfg(windows)]
pub async fn run_service() -> Result<()> {
    use windows_service::{
        service::{ServiceControl},
        service_control_handler::{self, ServiceControlHandlerResult}
    };
    use std::sync::Arc;

    init_logging()?;

    log::info!("Starting PatchPilot Windows service...");

    let running_flag = Arc::new(AtomicBool::new(true));
    let running_flag_clone = running_flag.clone();

    fn my_service_main(running_flag: Arc<AtomicBool>) -> Result<()> {
        let client = Client::new();
        let server_url = futures::executor::block_on(read_server_url())?;
        futures::executor::block_on(
            run_adoption_and_update_loop(&client, &server_url, Some(&running_flag))
        )?;
        Ok(())
    }

    fn service_control_handler(control: ServiceControl) -> ServiceControlHandlerResult {
        match control {
            ServiceControl::Stop => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    }

    let _status_handle =
        service_control_handler::register("PatchPilot", service_control_handler)?;

    my_service_main(running_flag_clone)
}
