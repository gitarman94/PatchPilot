// src/service.rs
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use serde_json::Value;
use std::{fs, time::Duration};
use crate::system_info::{SystemInfo, get_system_info}; // system_info provides synchronous helpers; you've added async helpers elsewhere
use local_ip_address::local_ip;
use tokio::time::{sleep, timeout};
use tokio::task;
use std::sync::atomic::{AtomicBool, Ordering};

const ADOPTION_CHECK_INTERVAL: u64 = 10;
const SYSTEM_UPDATE_INTERVAL: u64 = 600;

// Long-poll specifics
const COMMAND_LONGPOLL_TIMEOUT_SECS: u64 = 60; // server should hold request up to this many seconds
const COMMAND_RETRY_BACKOFF_SECS: u64 = 5;     // when poll fails, wait before retrying

#[cfg(any(unix, target_os = "macos"))]
const DEVICE_ID_FILE: &str = "/opt/patchpilot_client/device_id.txt";
#[cfg(windows)]
const DEVICE_ID_FILE: &str = "C:\\ProgramData\\PatchPilot\\device_id.txt";

#[cfg(any(unix, target_os = "macos"))]
const SERVER_URL_FILE: &str = "/opt/patchpilot_client/server_url.txt";
#[cfg(windows)]
const SERVER_URL_FILE: &str = "C:\\ProgramData\\PatchPilot\\server_url.txt";

pub fn init_logging() -> anyhow::Result<flexi_logger::LoggerHandle> {
    use flexi_logger::{
        Age, Cleanup, Criterion, FileSpec, Logger, Naming, WriteMode,
    };

    let base_dir = crate::get_base_dir();
    let log_dir = format!("{}/logs", base_dir);

    let _ = std::fs::create_dir_all(&log_dir);

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;

        if let Ok(meta) = std::fs::metadata(&log_dir) {
            let mut perms = meta.permissions();
            perms.set_mode(0o770);

            if std::fs::set_permissions(&log_dir, perms.clone()).is_err() {
                let mut fallback = perms;
                fallback.set_mode(0o777);
                let _ = std::fs::set_permissions(&log_dir, fallback);
            }

            if nix::unistd::Uid::effective().is_root() {
                let _ =
                    std::process::Command::new("chown")
                        .arg("-R")
                        .arg("patchpilot:patchpilot")
                        .arg(&log_dir)
                        .output();
            }
        }
    }

    let symlink_path = format!("{}/patchpilot_current.log", log_dir);

    let logger = Logger::try_with_str("info")?
        .log_to_file(
            FileSpec::default()
                .directory(&log_dir)
                .basename("patchpilot"),
        )
        .create_symlink(symlink_path)
        .write_mode(WriteMode::Direct)
        .duplicate_to_stderr(flexi_logger::Duplicate::None)
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(7),
        )
        .start()?;

    Ok(logger)
}
pub(crate) async fn read_server_url() -> Result<String> {
    use std::fs;
    let path = if cfg!(windows) {"C:\\ProgramData\\PatchPilot\\server_url.txt"} 
        else {"/opt/patchpilot_client/server_url.txt"};
    let url = fs::read_to_string(path)?.trim().to_string();
    Ok(url)
}

pub fn get_ip_address() -> String {
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

    // Collect system info properly (synchronous short path + refresh)
    let mut sys_info = match get_system_info() {
        Ok(info) => info,
        Err(_) => SystemInfo::gather_blocking(),
    };

    // Note: get_system_info() returns a fresh blocking snapshot in your current implementation.
    // If you have async helpers, you can call those instead.

    // Build JSON payload
    let payload = json!({
        "system_info": {
            "hostname": sys_info.hostname,
            "os_name": sys_info.os_name,
            "architecture": sys_info.architecture,
            "cpu_usage": sys_info.cpu_usage,
            "cpu_count": sys_info.cpu_count,
            "cpu_brand": sys_info.cpu_brand,
            "ram_total": sys_info.ram_total,
            "ram_used": sys_info.ram_used,
            "disk_total": sys_info.disk_total,
            "disk_free": sys_info.disk_free,
            "disk_health": sys_info.disk_health,
            "network_throughput": sys_info.network_throughput,
            "ping_latency": sys_info.ping_latency,
            "ip_address": sys_info.ip_address,
            "network_interfaces": sys_info.network_interfaces,
        },
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

    // Server returns { pending_id: "..." } or { device_id: "..." }
    let parsed: Value =
        serde_json::from_str(&body).context("Server returned invalid JSON")?;

    if let Some(pid) = parsed.get("pending_id").and_then(|v| v.as_str()) {
        write_local_device_id(pid)?;
        return Ok(pid.to_string());
    }
    if let Some(did) = parsed.get("device_id").and_then(|v| v.as_str()) {
        write_local_device_id(did)?;
        return Ok(did.to_string());
    }

    anyhow::bail!("Server did not return pending_id or device_id");
}

async fn send_system_update(
    client: &Client,
    server_url: &str,
    device_id: &str,
    device_type: &str,
    device_model: &str,
) -> Result<()> {

    let mut sys_info = match get_system_info() {
        Ok(info) => info,
        Err(_) => SystemInfo::gather_blocking(),
    };

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

async fn send_heartbeat(
    client: &Client,
    server_url: &str,
    device_id: &str,
    device_type: &str,
    device_model: &str,
) -> Result<Value> {
    // heartbeat returns server JSON (we will inspect it for adopted/status and commands optionally)
    let mut sys_info = get_system_info();
    };

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

/// Execute a single command (shell or script) in a blocking OS process and return result JSON.
async fn execute_command_and_post_result(
    client: Client,
    server_url: String,
    device_id: String,
    cmd_item: Value,
) {
    // Expect cmd_item to contain at least an "id" field and either "exec" (string) or "script" (string).
    let cmd_id = cmd_item.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
    if cmd_id.is_none() {
        log::warn!("Received command without id: {:?}", cmd_item);
        return;
    }
    let cmd_id = cmd_id.unwrap();

    let kind = cmd_item.get("kind").and_then(|v| v.as_str()).unwrap_or("exec");

    // Build the command to run
    let maybe_cmd_string = cmd_item
        .get("exec")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| cmd_item.get("script").and_then(|v| v.as_str()).map(|s| s.to_string()));

    let args_array = cmd_item.get("args").and_then(|v| v.as_array()).cloned();

    if maybe_cmd_string.is_none() {
        log::warn!("Command has no 'exec' or 'script' field: {:?}", cmd_item);
        // post error status back
        let _ = post_command_result(&client, &server_url, &device_id, &cmd_id, json!({
            "status": "error",
            "reason": "missing exec/script field"
        })).await;
        return;
    }

    let cmd_string = maybe_cmd_string.unwrap();

    // Run in blocking threadpool to avoid blocking runtime
    let run = task::spawn_blocking(move || {
        // Platform-specific shell invocation
        #[cfg(windows)]
        let out = {
            use std::process::Command;
            Command::new("cmd")
                .args(&["/C", &cmd_string])
                .output()
        };

        #[cfg(not(windows))]
        let out = {
            use std::process::Command;
            Command::new("sh")
                .arg("-c")
                .arg(&cmd_string)
                .output()
        };

        match out {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                let exit_code = o.status.code().unwrap_or(-1);
                (true, stdout, stderr, exit_code)
            }
            Err(e) => {
                (false, "".to_string(), format!("Failed to start process: {}", e), -1)
            }
        }
    }).await;

    match run {
        Ok((ok, stdout, stderr, exit_code)) => {
            let status = if ok { "ok" } else { "error" };
            let payload = json!({
                "status": status,
                "kind": kind,
                "stdout": stdout,
                "stderr": stderr,
                "exit_code": exit_code,
            });

            let _ = post_command_result(&client, &server_url, &device_id, &cmd_id, payload).await;
        }
        Err(e) => {
            log::error!("Command thread panicked: {}", e);
            let _ = post_command_result(&client, &server_url, &device_id, &cmd_id, json!({
                "status": "error",
                "reason": format!("panic: {}", e)
            })).await;
        }
    }
}

/// Post command result back to server
async fn post_command_result(
    client: &Client,
    server_url: &str,
    device_id: &str,
    cmd_id: &str,
    payload: Value,
) -> Result<()> {
    let url = format!("{}/api/devices/{}/commands/{}/result", server_url, device_id, cmd_id);

    let resp = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .context("Failed to POST command result")?;

    if !resp.status().is_success() {
        log::warn!("Server rejected command result {}: {}", cmd_id, resp.status());
    } else {
        log::info!("Posted result for command {}", cmd_id);
    }

    Ok(())
}

/// Long-poll loop: repeatedly ask server for commands and execute them.
/// This returns only when the running_flag becomes false (if provided) or an unrecoverable error occurs.
async fn command_longpoll_loop(
    client: Client,
    server_url: String,
    device_id: String,
    running_flag: Option<&AtomicBool>,
) {
    log::info!("Starting command long-poll loop for device {}", device_id);

    loop {
        if let Some(flag) = running_flag {
            if !flag.load(Ordering::SeqCst) {
                log::info!("Command long-poll stopping due to service shutdown flag");
                return;
            }
        }

        let poll_url = format!("{}/api/devices/{}/commands/poll", server_url, device_id);
        // We wrap the http request in tokio::time::timeout to enforce a long-poll window client-side.
        let req_future = client.get(&poll_url).send();

        match timeout(Duration::from_secs(COMMAND_LONGPOLL_TIMEOUT_SECS), req_future).await {
            Ok(Ok(resp)) => {
                // Got response from server (maybe empty list)
                if !resp.status().is_success() {
                    log::warn!("Command poll returned non-OK: {}", resp.status());
                    sleep(Duration::from_secs(COMMAND_RETRY_BACKOFF_SECS)).await;
                    continue;
                }

                match resp.json::<Value>().await {
                    Ok(val) => {
                        // Expecting an array of commands
                        if let Some(arr) = val.as_array() {
                            if arr.is_empty() {
                                // No commands; immediately loop to re-poll
                                continue;
                            }

                            // For each command, spawn a task to execute and report
                            for cmd_item in arr.iter() {
                                // Clone client/server/device for the spawned task
                                let client_clone = client.clone();
                                let server_clone = server_url.clone();
                                let device_clone = device_id.clone();
                                let cmd_clone = cmd_item.clone();
                                tokio::spawn(async move {
                                    execute_command_and_post_result(client_clone, server_clone, device_clone, cmd_clone).await;
                                });
                            }
                        } else {
                            log::warn!("Unexpected command poll response (not array): {:?}", val);
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to parse command poll JSON: {}", e);
                        sleep(Duration::from_secs(COMMAND_RETRY_BACKOFF_SECS)).await;
                    }
                }
            }
            Ok(Err(e)) => {
                // HTTP error
                log::warn!("Command poll HTTP error: {}", e);
                sleep(Duration::from_secs(COMMAND_RETRY_BACKOFF_SECS)).await;
            }
            Err(_) => {
                // Timeout (long-poll duration elapsed) -> simply re-loop to poll again
                // This is expected behavior; server may hold request up to longpoll timeout.
                continue;
            }
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

    // Ensure adoption via heartbeat
    loop {
        match send_heartbeat(client, server_url, &device_id, &device_type, &device_model).await {
            Ok(v) => {
                let adopted = v.get("adopted").and_then(|x| x.as_bool()).unwrap_or(false)
                    || v.get("status").and_then(|x| x.as_str()) == Some("adopted");
                if adopted {
                    break;
                } else {
                    log::info!("Device not yet adopted; heartbeat returned {:?}", v);
                }
            }
            Err(e) => {
                log::warn!("Heartbeat failed while waiting for adoption: {}", e);
            }
        }
        sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL)).await;
    }

    // Launch command long-poll loop as a background task so updates & heartbeats continue.
    // Provide a clone of client and server_url. The loop will observe `running_flag`.
    let client_for_poller = client.clone();
    let server_url_string = server_url.to_string();
    let device_id_string = device_id.clone();
    let flag_for_poller = running_flag.map(|f| f as *const AtomicBool); // raw pointer for move into closure

    // spawn background poller
    let poller_handle = {
        let running_flag_owned = running_flag;
        tokio::spawn(async move {
            command_longpoll_loop(client_for_poller, server_url_string, device_id_string, running_flag_owned).await;
        })
    };

    // Main periodic system update loop (keeps heartbeat & updates)
    loop {
        if let Some(flag) = running_flag {
            if !flag.load(Ordering::SeqCst) {
                log::info!("Shutting down update loop due to flag");
                break;
            }
        }

        if let Err(e) = send_system_update(client, server_url, &device_id, &device_type, &device_model).await {
            log::warn!("system_update failed: {}", e);
        }

        // Heartbeat (also lets server send back immediate commands or status if you choose)
        match send_heartbeat(client, server_url, &device_id, &device_type, &device_model).await {
            Ok(v) => {
                log::debug!("Heartbeat OK: {:?}", v);
            }
            Err(e) => {
                log::warn!("Heartbeat failed: {}", e);
            }
        }

        sleep(Duration::from_secs(SYSTEM_UPDATE_INTERVAL)).await;
    }

    // When shutting down, allow poller to stop (it checks running_flag)
    if let Ok(join) = poller_handle.await {
        let _ = join;
    }

    Ok(())
}

#[cfg(any(unix, target_os = "macos"))]
pub async fn run_unix_service() -> Result<()> {
    let client = Client::new();
    let server_url = read_server_url().await?;
    run_adoption_and_update_loop(&client, &server_url, None).await
}

#[cfg(windows)]
pub async fn run_service() -> Result<()> {
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
