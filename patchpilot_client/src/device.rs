use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::{Arc, atomic::AtomicBool};
use tokio::time::sleep;
use std::time::Duration;
use crate::system_info::{SystemInfo, get_system_info, get_local_device_id, write_local_device_id, get_device_info_basic};

pub const ADOPTION_CHECK_INTERVAL: u64 = 10;
pub const SYSTEM_UPDATE_INTERVAL: u64 = 600;

// Register the device with the server
pub async fn register_device(
    client: &Client,
    server_url: &str,
    device_type: &str,
    device_model: &str,
) -> Result<String> {
    let sys_info: SystemInfo = get_system_info();

    // Load or generate a persistent device UUID
    let device_uuid = match get_local_device_id() {
        Some(id) => id,
        None => {
            let device_id = response.device_id.clone();
            write_device_id(&device_id)?;
            return Ok(device_id);
        }
    };

    // Payload MUST match server DeviceInfo exactly
    let payload = json!({
        "uuid": device_uuid,
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

// Send a system update to the server
pub async fn send_system_update(
    client: &Client,
    server_url: &str,
    device_id: &str,
    device_type: &str,
    device_model: &str,
) -> Result<()> {
    let sys_info: SystemInfo = get_system_info();

    let payload = json!({
        "system_info": sys_info,
        "device_type": device_type,
        "device_model": device_model
    });

    let resp = client
        .post(format!("{}/api/devices/{}", server_url, device_id))
        .json(&payload)
        .send()
        .await
        .context("Update request failed")?;

    if !resp.status().is_success() {
        anyhow::bail!("Server update rejected: {}", resp.status());
    }

    Ok(())
}

// Send a heartbeat to the server and return JSON
pub async fn send_heartbeat(
    client: &Client,
    server_url: &str,
    device_id: &str,
    device_type: &str,
    device_model: &str,
) -> Result<Value> {
    let sys_info: SystemInfo = get_system_info();

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

// Run full adoption & update loop
pub async fn run_adoption_and_update_loop(
    client: &Client,
    server_url: &str,
    running_flag: Option<Arc<AtomicBool>>,
) -> Result<String> {
    let (device_type, device_model) = get_device_info_basic();
    let mut device_id = get_local_device_id();

    // Register device if we don't already have an ID
    if device_id.is_none() {
        loop {
            if let Some(flag) = &running_flag {
                if !flag.load(Ordering::SeqCst) {
                    return Err(anyhow!("Service stopping during device registration"));
                }
            }

            match register_device(client, server_url, &device_type, &device_model).await {
                Ok(id) => {
                    device_id = Some(id.clone());
                    break;
                }
                Err(e) => {
                    log::warn!("No device_id yet, retrying...: {}", e);
                    sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL)).await;
                }
            }
        }
    }

    let device_id = device_id.expect("device_id must be present after registration");

    // Wait until server reports device is adopted
    loop {
        if let Some(flag) = &running_flag {
            if !flag.load(Ordering::SeqCst) {
                return Err(anyhow!("Service stopping during adoption wait"));
            }
        }

        match send_heartbeat(client, server_url, &device_id, &device_type, &device_model).await {
            Ok(v) => {
                let adopted =
                    v.get("adopted").and_then(|x| x.as_bool()).unwrap_or(false)
                    || v.get("status").and_then(|x| x.as_str()) == Some("adopted");

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

