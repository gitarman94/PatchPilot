use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use tokio::time::{sleep, Duration};
use crate::system_info::{
    SystemInfo, SystemInfoService, get_local_device_id, write_local_device_id, get_device_info_basic
};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Instant;

pub const ADOPTION_CHECK_INTERVAL: i64 = 10;

// Helper: measure TCP ping (ms) to host:port
fn measure_tcp_ping(host: &str, port: u16, timeout_ms: i64) -> Option<f32> {
    let addr = format!("{}:{}", host, port);
    let addr = addr.to_socket_addrs().ok()?.next()?;
    let start = Instant::Utc::now();
    let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms)).ok()?;
    Some(start.elapsed().as_secs_f32() * 1000.0)
}

// Register the device with the server
pub async fn register_device(
    client: &Client,
    server_url: &str,
    device_type: &str,
    device_model: &str,
    system_info_service: &Arc<SystemInfoService>,
) -> Result<String> {
    let mut sys_info: SystemInfo = system_info_service.get_system_info_async().await.unwrap_or_else(|_| get_local_device_id().map(|_| SystemInfo::default()).unwrap_or_default());

    // Read stored device ID if available
    let device_id = get_local_device_id().unwrap_or_default();

    let payload = json!({
        "device_id": device_id,
        "system_info": sys_info,
        "device_type": device_type,
        "device_model": device_model
    });

    let url = format!("{}/api/register", server_url);
    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .context("Error sending registration request")?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if !status.is_success() {
        anyhow::bail!("Registration failed {}: {}", status, body);
    }

    let parsed: Value =
        serde_json::from_str(&body).context("Server returned invalid JSON")?;

    if let Some(did) = parsed.get("device_id").and_then(|v| v.as_str()) {
        write_local_device_id(did)?;
        return Ok(did.to_string());
    }

    anyhow::bail!("Server did not return device_id");
}

// Send heartbeat to server
pub async fn send_heartbeat(
    client: &Client,
    server_url: &str,
    device_id: &str,
    device_type: &str,
    device_model: &str,
    system_info_service: &Arc<SystemInfoService>,
) -> Result<Value> {
    let mut sys_info = system_info_service.get_system_info_async().await.unwrap_or_default();

    let payload = json!({
        "device_id": device_id,
        "system_info": sys_info,
        "device_type": device_type,
        "device_model": device_model
    });

    let resp = client
        .post(format!("{}/api/devices/heartbeat", server_url))
        .json(&payload)
        .send()
        .await
        .context("Heartbeat request failed")?;

    if !resp.status().is_success() {
        anyhow::bail!("Heartbeat request rejected: {}", resp.status());
    }

    let v = resp.json::<Value>().await.context("Parsing heartbeat response JSON")?;
    Ok(v)
}

// Run adoption & update loop
pub async fn run_adoption_and_update_loop(
    client: &Client,
    server_url: &str,
    running_flag: Option<Arc<AtomicBool>>,
) -> Result<String> {
    let (device_type, device_model) = get_device_info_basic();
    let mut device_id = get_local_device_id();
    let system_info_service = Arc::new(SystemInfoService::default());

    // Register device if none
    if device_id.is_none() {
        loop {
            if let Some(flag) = &running_flag {
                if !flag.load(Ordering::SeqCst) {
                    return Err(anyhow!("Service stopping during device registration"));
                }
            }

            match register_device(client, server_url, &device_type, &device_model, &system_info_service).await {
                Ok(id) => {
                    device_id = Some(id.clone());
                    break;
                }
                Err(e) => {
                    log::warn!("Registration retry: {}", e);
                    sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL)).await;
                }
            }
        }
    }

    let device_id = device_id.expect("device_id missing after registration");

    // Heartbeat until adopted
    loop {
        if let Some(flag) = &running_flag {
            if !flag.load(Ordering::SeqCst) {
                return Err(anyhow!("Service stopping during adoption wait"));
            }
        }

        match send_heartbeat(client, server_url, &device_id, &device_type, &device_model, &system_info_service).await {
            Ok(v) => {
                let adopted = v.get("adopted").and_then(|x| x.as_bool()).unwrap_or(false);
                if adopted {
                    break;
                }
            }
            Err(e) => {
                log::warn!("Heartbeat failed: {}", e);
            }
        }

        sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL)).await;
    }

    Ok(device_id)
}
