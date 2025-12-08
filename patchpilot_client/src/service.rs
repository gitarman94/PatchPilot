use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use std::{fs, time::Duration};
use crate::system_info::{SystemInfo, get_system_info};
use local_ip_address::local_ip;
use tokio::time::sleep;
use std::sync::atomic::{AtomicBool, Ordering};

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

pub fn init_logging() -> anyhow::Result<()> {
    use flexi_logger::{Age, Cleanup, Criterion, Duplicate, FileSpec,
                       Logger, Naming, WriteMode};

    let base_dir = crate::get_base_dir();
    let log_dir = format!("{}/logs", base_dir);
    let _ = std::fs::create_dir_all(&log_dir);

    Logger::try_with_str("info")?
        .log_to_file(
            FileSpec::default()
                .directory(&log_dir)
                .basename("client")
        )
        .write_mode(WriteMode::Direct)
        .duplicate_to_stderr(Duplicate::Info)
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(7),
        )
        .start()
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("Failed to start logger: {}", e))
}

async fn read_server_url() -> Result<String> {
    let url = fs::read_to_string(SERVER_URL_FILE)
        .with_context(|| format!("Failed to read server URL from {}", SERVER_URL_FILE))?;
    Ok(url.trim().to_string())
}

fn get_ip_address() -> String {
    local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "0.0.0.0".into())
}

fn get_local_device_id() -> Option<String> {
    fs::read_to_string(DEVICE_ID_FILE).ok().map(|s| s.trim().to_string())
}

fn write_local_device_id(device_id: &str) -> Result<()> {
    fs::write(DEVICE_ID_FILE, device_id).context("Failed to write local device_id")
}

fn get_device_info_basic() -> (String, String) {
    match get_system_info() {
        Ok(info) => {
            // device_type and device_model are now Strings
            let device_type =
                if info.device_type.trim().is_empty() { "".into() } else { info.device_type };
            let device_model =
                if info.device_model.trim().is_empty() { "".into() } else { info.device_model };
            (device_type, device_model)
        }
        Err(_) => ("".into(), "".into()),
    }
}

async fn register_device(
    client: &Client,
    server_url: &str,
    device_type: &str,
    device_model: &str,
) -> Result<String> {

    let mut sys_info = match get_system_info() {
        Ok(info) => info,
        Err(_) => SystemInfo::new(),
    };
    sys_info.refresh();

    let resp = client
        .post(format!("{}/api/register", server_url))
        .json(&json!({
            "device_type": device_type,
            "device_model": device_model,
            "ip_address": get_ip_address(),
            "system_info": sys_info
        }))
        .send()
        .await
        .context("Registration request failed")?;

    let json_resp: serde_json::Value =
        resp.json().await.context("Invalid JSON from server")?;

    if let Some(id) = json_resp.get("device_id").and_then(|v| v.as_str()) {
        write_local_device_id(id)?;
        return Ok(id.to_string());
    }

    if let Some(pending) = json_resp.get("pending_id").and_then(|v| v.as_str()) {
        write_local_device_id(pending)?;
        return Ok(pending.to_string());
    }

    anyhow::bail!("Server did not return device_id or pending_id")
}

async fn send_system_update(
    client: &Client,
    server_url: &str,
    device_id: &str,
) -> Result<()> {

    let mut sys_info = match get_system_info() {
        Ok(info) => info,
        Err(_) => SystemInfo::new(),
    };
    sys_info.refresh();

    let payload = json!({
        "device_id": device_id,
        "system_info": sys_info
    });

    let resp = client
        .post(format!("{}/api/update", server_url))
        .json(&payload)
        .send()
        .await
        .context("Update request failed")?;

    if !resp.status().is_success() {
        anyhow::bail!("Server update rejected: {}", resp.status());
    }

    Ok(())
}

async fn send_heartbeat(
    client: &Client,
    server_url: &str,
    device_id: &str,
    device_type: &str,
    device_model: &str,
) -> bool {

    let mut sys_info = match get_system_info() {
        Ok(info) => info,
        Err(_) => {
            let mut blank = SystemInfo::new();
            blank.refresh();
            blank
        }
    };

    sys_info.refresh();

    let payload = serde_json::json!({
        "device_id": device_id,
        "device_type": device_type,
        "device_model": device_model,
        "system_info": sys_info,
        "ip_address": get_ip_address()
    });

    let resp = client
        .post(format!("{}/api/devices/heartbeat", server_url))
        .json(&payload)
        .send()
        .await;

    match resp {
        Ok(r) => {
            if !r.status().is_success() {
                log::warn!("Heartbeat request returned non-OK: {}", r.status());
                return false;
            }
            match r.json::<serde_json::Value>().await {
                Ok(v) => {
                    v.get("adopted").and_then(|x| x.as_bool()).unwrap_or(false)
                        || v.get("status").and_then(|x| x.as_str()) == Some("adopted")
                }
                Err(e) => {
                    log::warn!("Failed to parse heartbeat JSON response: {}", e);
                    false
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to send heartbeat: {}", e);
            false
        }
    }
}

async fn run_adoption_and_update_loop(
    client: &Client,
    server_url: &str,
    running_flag: Option<&AtomicBool>
) -> Result<()> {

    let (device_type, device_model) = get_device_info_basic();
    let mut device_id = get_local_device_id();

    if device_id.is_none() {
        loop {
            match register_device(client, server_url, &device_type, &device_model).await {
                Ok(id) => {
                    log::info!("Received device_id from server: {}", id);
                    write_local_device_id(&id)?;
                    device_id = Some(id);
                    break;
                }
                Err(e) => {
                    log::warn!("No device_id yet (server has not approved?). Retrying...: {}", e);
                    sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL)).await;
                }
            }
        }
    }

    let device_id = device_id.unwrap();

    loop {
        if send_heartbeat(client, server_url, &device_id, &device_type, &device_model).await {
            break;
        }
        sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL)).await;
    }

    loop {
        if let Some(flag) = running_flag {
            if !flag.load(Ordering::SeqCst) {
                return Ok(());
            }
        }

        if let Err(e) = send_system_update(client, server_url, &device_id).await {
            log::warn!("system_update failed: {}", e);
        }

        sleep(Duration::from_secs(SYSTEM_UPDATE_INTERVAL)).await;
    }
}

#[cfg(any(unix, target_os = "macos"))]
pub async fn run_unix_service() -> Result<()> {
    init_logging()?;
    let client = Client::new();
    let server_url = read_server_url().await?;
    run_adoption_and_update_loop(&client, &server_url, None).await
}

#[cfg(windows)]
pub async fn run_service() -> Result<()> {
    init_logging()?;
    use windows_service::{
        service::{ServiceControl, ServiceControlHandlerResult},
        service_control_handler,
    };
    use std::sync::Arc;

    let running_flag = Arc::new(AtomicBool::new(true));
    let running_flag_clone = running_flag.clone();

    fn service_main(flag: Arc<AtomicBool>) -> Result<()> {
        let client = Client::new();
        let server_url = futures::executor::block_on(read_server_url())?;
        futures::executor::block_on(run_adoption_and_update_loop(
            &client,
            &server_url,
            Some(&flag),
        ))
    }

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

    service_main(running_flag_clone)
}
