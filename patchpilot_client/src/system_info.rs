use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fs, time::Duration};
use crate::system_info::{SystemInfo, get_system_info, SystemInfoService};
use local_ip_address::local_ip;
use tokio::time::sleep;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::timeout;
use std::process::Stdio;

const ADOPTION_CHECK_INTERVAL: u64 = 10;
const SYSTEM_UPDATE_INTERVAL: u64 = 600;
const COMMAND_POLL_INTERVAL: u64 = 5; // seconds between polls for commands (server recommended small)
const COMMAND_DEFAULT_TIMEOUT_SECS: u64 = 300; // default timeout for running commands (5 minutes)

#[cfg(any(unix, target_os = "macos"))]
const DEVICE_ID_FILE: &str = "/opt/patchpilot_client/device_id.txt";
#[cfg(windows)]
const DEVICE_ID_FILE: &str = "C:\\ProgramData\\PatchPilot\\device_id.txt";

#[cfg(any(unix, target_os = "macos"))]
const SERVER_URL_FILE: &str = "/opt/patchpilot_client/server_url.txt";
#[cfg(windows)]
const SERVER_URL_FILE: &str = "C:\\ProgramData\\PatchPilot\\server_url.txt";

#[cfg(any(unix, target_os = "macos"))]
const SCRIPTS_DIR: &str = "/opt/patchpilot_client/scripts";
#[cfg(windows)]
const SCRIPTS_DIR: &str = "C:\\ProgramData\\PatchPilot\\scripts";

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum CommandSpec {
    #[serde(rename = "shell")]
    Shell { command: String, timeout_secs: Option<u64> },

    #[serde(rename = "script")]
    Script { name: String, args: Option<Vec<String>>, timeout_secs: Option<u64> },
}

/// Server's representation of an enqueued command.
#[derive(Serialize, Deserialize, Debug)]
pub struct ServerCommand {
    pub id: String,
    pub spec: CommandSpec,
    pub created_at: Option<String>,
    pub run_as_root: Option<bool>,
}

/// Result object to post back to server for a command
#[derive(Serialize, Deserialize, Debug)]
struct CommandResult {
    pub id: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_secs: f64,
    pub success: bool,
}

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

async fn read_server_url() -> Result<String> {
    let url = fs::read_to_string(SERVER_URL_FILE)
        .with_context(|| format!("Failed to read server URL from {}", SERVER_URL_FILE))?;
    Ok(url.trim().to_string())
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

    // Collect system info properly (use async rate-limited gather)
    let svc = SystemInfoService::default();
    let sys_info = svc.get_system_info_async().await.unwrap_or_else(|_| SystemInfo::gather_blocking());

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
        "device_model": device_model,
        "capabilities": ["shell","script-run","sysinfo"]
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

    let parsed: serde_json::Value =
        serde_json::from_str(&body).context("Server returned invalid JSON")?;

    if let Some(pid) = parsed["pending_id"].as_str() {
        write_local_device_id(pid)?;
        return Ok(pid.to_string());
    }

    anyhow::bail!("Server did not return pending_id");
}


async fn send_system_update(
    client: &Client,
    server_url: &str,
    device_id: &str,
    device_type: &str,
    device_model: &str,
) -> Result<()> {

    // Use the default async SystemInfoService to get fresh data (with lightweight rate-limit)
    let svc = SystemInfoService::default();
    let sys_info = svc.get_system_info_async().await.unwrap_or_else(|_| SystemInfo::gather_blocking());

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
) -> bool {

    let svc = SystemInfoService::default();
    let sys_info = svc.get_system_info_async().await.unwrap_or_else(|_| SystemInfo::gather_blocking());

    // Build heartbeat payload
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

/// Poll the server for pending commands for this device once.
/// Expects server to return JSON array of `ServerCommand` objects.
/// This is intentionally lightweight and predictable.
async fn poll_for_commands_once(client: &Client, server_url: &str, device_id: &str) -> Result<Vec<ServerCommand>> {
    let resp = client
        .get(format!("{}/api/devices/{}/commands/poll", server_url, device_id))
        .send()
        .await
        .context("Failed to poll commands")?;

    if !resp.status().is_success() {
        anyhow::bail!("Command poll failed: {}", resp.status());
    }

    let commands: Vec<ServerCommand> = resp.json().await.context("Invalid command JSON")?;
    Ok(commands)
}

/// Execute a single command spec and return the result struct.
/// This runs the command inside spawn_blocking to avoid blocking the async runtime and applies a timeout.
async fn execute_command_and_collect_result(cmd: &ServerCommand) -> CommandResult {
    let start = std::time::Instant::now();

    // Default timeout if not specified
    let timeout_secs = match &cmd.spec {
        CommandSpec::Shell { timeout_secs, .. } => timeout_secs.unwrap_or(COMMAND_DEFAULT_TIMEOUT_SECS),
        CommandSpec::Script { timeout_secs, .. } => timeout_secs.unwrap_or(COMMAND_DEFAULT_TIMEOUT_SECS),
    };

    // Run blocking work inside spawn_blocking with Tokio timeout wrapper.
    let spec_clone = cmd.spec.clone();
    let id_clone = cmd.id.clone();

    let run = tokio::task::spawn_blocking(move || {
        // This closure is executed on a blocking thread.
        match spec_clone {
            CommandSpec::Shell { command, .. } => {
                // On Unix use /bin/sh -c, on Windows use cmd /C or powershell as needed
                #[cfg(unix)]
                {
                    let mut c = std::process::Command::new("/bin/sh");
                    c.arg("-c").arg(command);
                    c.stdin(Stdio::null());
                    c.stdout(Stdio::piped());
                    c.stderr(Stdio::piped());
                    let out = c.output();
                    return out.map_err(|e| format!("failed spawn: {}", e));
                }
                #[cfg(windows)]
                {
                    // Use PowerShell for improved compatibility
                    let mut c = std::process::Command::new("powershell");
                    c.arg("-NoProfile").arg("-NonInteractive").arg("-Command").arg(command);
                    c.stdin(Stdio::null());
                    c.stdout(Stdio::piped());
                    c.stderr(Stdio::piped());
                    let out = c.output();
                    return out.map_err(|e| format!("failed spawn: {}", e));
                }
            }
            CommandSpec::Script { name, args, .. } => {
                // Resolve script path in SCRIPTS_DIR; do basic validation
                let script_path = {
                    #[cfg(any(unix, target_os = "macos"))]
                    {
                        std::path::PathBuf::from(format!("{}/{}", SCRIPTS_DIR, name))
                    }
                    #[cfg(windows)]
                    {
                        std::path::PathBuf::from(format!("{}\\{}", SCRIPTS_DIR, name))
                    }
                };

                if !script_path.exists() {
                    return Err(format!("script not found: {:?}", script_path));
                }

                let mut c = std::process::Command::new(script_path);
                if let Some(argsv) = args {
                    for a in argsv {
                        c.arg(a);
                    }
                }
                c.stdin(Stdio::null());
                c.stdout(Stdio::piped());
                c.stderr(Stdio::piped());
                let out = c.output();
                return out.map_err(|e| format!("failed spawn: {}", e));
            }
        }
    });

    // wait with timeout
    let output_res = timeout(Duration::from_secs(timeout_secs), run).await;

    let duration = start.elapsed();
    match output_res {
        Ok(join_res) => {
            match join_res {
                Ok(Ok(os_output)) => {
                    let stdout = String::from_utf8_lossy(&os_output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&os_output.stderr).to_string();
                    let code = os_output.status.code().unwrap_or(-1);
                    CommandResult {
                        id: id_clone,
                        exit_code: code,
                        stdout,
                        stderr,
                        duration_secs: duration.as_secs_f64(),
                        success: os_output.status.success(),
                    }
                }
                Ok(Err(err_str)) => {
                    CommandResult {
                        id: id_clone,
                        exit_code: -1,
                        stdout: "".into(),
                        stderr: format!("spawn error: {}", err_str),
                        duration_secs: duration.as_secs_f64(),
                        success: false,
                    }
                }
                Err(join_err) => {
                    CommandResult {
                        id: id_clone,
                        exit_code: -1,
                        stdout: "".into(),
                        stderr: format!("join error: {:?}", join_err),
                        duration_secs: duration.as_secs_f64(),
                        success: false,
                    }
                }
            }
        }
        Err(_) => {
            // timeout triggered — attempt to best-effort kill is not always possible because process is on blocking thread.
            CommandResult {
                id: id_clone,
                exit_code: -1,
                stdout: "".into(),
                stderr: format!("command timed out after {}s", timeout_secs),
                duration_secs: duration.as_secs_f64(),
                success: false,
            }
        }
    }
}

/// Post command result back to server
async fn post_command_result(client: &Client, server_url: &str, result: &CommandResult) -> Result<()> {
    let resp = client
        .post(format!("{}/api/commands/{}/result", server_url, result.id))
        .json(result)
        .send()
        .await
        .context("Failed to post command result")?;

    if !resp.status().is_success() {
        anyhow::bail!("Server rejected result: {}", resp.status());
    }
    Ok(())
}

/// Poll / execute / report loop (runs concurrently with the adoption/update logic).
/// This loop is tolerant of network errors and uses a short interval between polls.
async fn command_poll_loop(client: Client, server_url: String, device_id: String, running_flag: Option<&AtomicBool>) {
    loop {
        if let Some(flag) = running_flag {
            if !flag.load(Ordering::SeqCst) {
                log::info!("Command poll loop stopping due to service stop signal.");
                return;
            }
        }

        match poll_for_commands_once(&client, &server_url, &device_id).await {
            Ok(commands) => {
                if !commands.is_empty() {
                    log::info!("Received {} commands to execute", commands.len());
                }
                for cmd in commands.into_iter() {
                    // Execute each command and post result (don't let one fail block others)
                    let r = execute_command_and_collect_result(&cmd).await;
                    if let Err(e) = post_command_result(&client, &server_url, &r).await {
                        log::warn!("Failed to post command result for {}: {}", r.id, e);
                    }
                }
            }
            Err(e) => {
                log::warn!("Command poll failed: {}", e);
            }
        }

        sleep(Duration::from_secs(COMMAND_POLL_INTERVAL)).await;
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

    // keep checking heartbeat until adopted
    loop {
        if send_heartbeat(client, server_url, &device_id, &device_type, &device_model).await {
            break;
        }
        sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL)).await;
    }

    // start command poll loop in background (if desired)
    {
        let client_clone = client.clone();
        let server_clone = server_url.to_string();
        let device_clone = device_id.clone();
        // spawn a detached task for command polling; if service is stopped, we'll check running_flag inside.
        let running_flag_clone = running_flag.map(|f| f as *const AtomicBool);
        tokio::spawn(async move {
            let rf: Option<&AtomicBool> = running_flag_clone.map(|p| unsafe { &*p });
            command_poll_loop(client_clone, server_clone, device_clone, rf).await;
        });
    }

    // Main update loop
    loop {
        if let Some(flag) = running_flag {
            if !flag.load(Ordering::SeqCst) {
                return Ok(());
            }
        }

        if let Err(e) = send_system_update(client, server_url, &device_id, &device_type, &device_model).await {
            log::warn!("system_update failed: {}", e);
        }

        sleep(Duration::from_secs(SYSTEM_UPDATE_INTERVAL)).await;
    }
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
