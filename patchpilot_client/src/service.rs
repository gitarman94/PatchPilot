use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use std::{fs, time::Duration};
use crate::system_info::{SystemInfo, get_system_info};
use log::{info, error};
use local_ip_address::local_ip;
use tokio::time::sleep;
use std::sync::atomic::{AtomicBool, Ordering};

const ADOPTION_CHECK_INTERVAL: u64 = 30;
const SYSTEM_UPDATE_INTERVAL: u64 = 600;

async fn read_server_url() -> Result<String> {
    #[cfg(unix)]
    let path = "/opt/patchpilot_client/server_url.txt";
    #[cfg(windows)]
    let path = "C:\\ProgramData\\PatchPilot\\server_url.txt";

    let url = fs::read_to_string(path)
        .with_context(|| format!("Failed to read the server URL from {path}"))?;
    Ok(url.trim().to_string())
}

fn get_device_info() -> (String, String, String) {
    match get_system_info() {
        Ok(info) => {
            let device_id = info.serial_number.clone().unwrap_or_else(|| "unknown".into());
            let device_type = info.device_type.clone().unwrap_or_else(|| "unknown".into());
            let device_model = info.device_model.clone().unwrap_or_else(|| "unknown".into());
            (device_id, device_type, device_model)
        }
        Err(e) => {
            error!("Failed to gather device info: {:?}", e);
            ("unknown".into(), "unknown".into(), "unknown".into())
        }
    }
}

fn get_ip_address() -> String {
    local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "0.0.0.0".into())
}

async fn check_adoption_status(
    client: &Client,
    server_url: &str,
    device_id: &str,
    device_type: &str,
    device_model: &str,
) -> Result<bool> {
    let resp = client
        .post(format!("{}/api/devices/heartbeat", server_url))
        .json(&json!({
            "device_id": device_id,
            "device_type": device_type,
            "device_model": device_model,
            "ip_address": get_ip_address(),
            "network_interfaces": "eth0,wlan0",
        }))
        .send()
        .await;

    match resp {
        Ok(resp) if resp.status().is_success() => {
            let status_json: serde_json::Value = resp.json().await?;
            Ok(status_json.get("adopted").and_then(|v| v.as_bool()).unwrap_or(false))
        }
        Ok(resp) => {
            let text = resp.text().await.unwrap_or_default();
            error!("Unexpected HTTP {} from adoption check: {:?}", resp.status(), text);
            Ok(false)
        }
        Err(e) => {
            error!("Error during adoption check: {:?}", e);
            Ok(false)
        }
    }
}

async fn send_system_update(client: &Client, server_url: &str, device_id: &str) {
    let mut sys_info = match get_system_info() {
        Ok(info) => info,
        Err(e) => {
            error!("Failed to gather system info: {:?}", e);
            SystemInfo::new()
        }
    };

    sys_info.refresh();

    info!("Sending system update for device {}...", device_id);

    if let Err(e) = client
        .post(format!("{}/api/devices/{}", server_url, device_id))
        .json(&json!({
            "device_id": device_id,
            "status": "active",
            "system_info": sys_info,
            "ip_address": get_ip_address(),
        }))
        .send()
        .await
    {
        error!("Failed to send system update: {:?}", e);
    }
}

/// Shared adoption + system update loop (async)
async fn run_adoption_and_update_loop(
    client: &Client,
    server_url: &str,
    device_id: &str,
    device_type: &str,
    device_model: &str,
    running_flag: Option<&AtomicBool>
) {
    let mut adopted = false;

    while !adopted {
        if let Some(flag) = running_flag {
            if !flag.load(Ordering::SeqCst) {
                info!("Stopping adoption loop due to service stop signal.");
                return;
            }
        }

        match check_adoption_status(client, server_url, device_id, device_type, device_model).await {
            Ok(true) => {
                info!("Device adopted.");
                adopted = true;
            }
            Ok(false) => {
                info!("Device not adopted yet, retrying in {} seconds...", ADOPTION_CHECK_INTERVAL);
            }
            Err(e) => {
                error!("Adoption check failed: {:?}", e);
            }
        }

        sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL)).await;
    }

    loop {
        if let Some(flag) = running_flag {
            if !flag.load(Ordering::SeqCst) {
                info!("Stopping system update loop due to service stop signal.");
                break;
            }
        }

        send_system_update(client, server_url, device_id).await;
        sleep(Duration::from_secs(SYSTEM_UPDATE_INTERVAL)).await;
    }
}

// --- Unix service ---
#[cfg(unix)]
pub async fn run_unix_service() -> Result<()> {
    info!("Starting PatchPilot Unix service...");

    let client = Client::new();
    let server_url = read_server_url().await?;
    let (device_id, device_type, device_model) = get_device_info();

    run_adoption_and_update_loop(&client, &server_url, &device_id, &device_type, &device_model, None).await;

    Ok(())
}

// --- Windows service ---
#[cfg(windows)]
pub async fn run_service() -> Result<()> {
    use windows_service::{
        define_windows_service, service_dispatcher,
        service_control_handler::{ServiceControl, ServiceControlHandlerResult},
        service::{ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType},
    };

    define_windows_service!(ffi_service_main, service_entry);

    static SERVICE_RUNNING: AtomicBool = AtomicBool::new(true);

    info!("Starting PatchPilot Windows service...");
    service_dispatcher::start("PatchPilotService", ffi_service_main)?;

    async fn run() -> Result<()> {
        let status_handle = windows_service::service_control_handler::register(
            "PatchPilotService",
            move |control_event| match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    info!("Service stop signal received.");
                    SERVICE_RUNNING.store(false, Ordering::SeqCst);
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            },
        )?;

        let mut status = ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::from_secs(0),
            process_id: None,
        };
        status_handle.set_service_status(status)?;

        let client = Client::new();
        let server_url = read_server_url().await?;
        let (device_id, device_type, device_model) = get_device_info();

        run_adoption_and_update_loop(
            &client,
            &server_url,
            &device_id,
            &device_type,
            &device_model,
            Some(&SERVICE_RUNNING),
        ).await;

        info!("Service stopped.");
        status.current_state = ServiceState::Stopped;
        status_handle.set_service_status(status)?;
        Ok(())
    }

    tokio::spawn(run()).await??;

    Ok(())
}

#[cfg(unix)]
pub use run_unix_service;
#[cfg(windows)]
pub use run_service;
