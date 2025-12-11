use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fs, time::Duration};
use local_ip_address::local_ip;
use tokio::time::{sleep, timeout};
use std::process::Stdio;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use sysinfo::{System, Processor, Disk, Network, SystemExt};

/// Intervals (defaults). Server can override refresh interval by sending a config value
/// in heartbeat response; client can call `set_system_info_refresh_secs(...)`.
const ADOPTION_CHECK_INTERVAL: u64 = 10;
const DEFAULT_SYSTEM_UPDATE_INTERVAL: u64 = 600;
const DEFAULT_COMMAND_POLL_INTERVAL: u64 = 5;
const COMMAND_DEFAULT_TIMEOUT_SECS: u64 = 300;

/// Path constants (platform-specific).
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

/// Runtime-configurable refresh interval for SystemInfo async cache (seconds).
static SYSTEM_INFO_REFRESH_SECS: AtomicU64 = AtomicU64::new(10);

/// Public helper to let other modules change the refresh interval.
pub fn set_system_info_refresh_secs(secs: u64) {
    SYSTEM_INFO_REFRESH_SECS.store(if secs == 0 { 10 } else { secs }, Ordering::SeqCst);
}

fn get_system_info_refresh_secs() -> u64 {
    SYSTEM_INFO_REFRESH_SECS.load(Ordering::SeqCst)
}

/// Command spec from server.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum CommandSpec {
    #[serde(rename = "shell")]
    Shell { command: String, timeout_secs: Option<u64> },

    #[serde(rename = "script")]
    Script { name: String, args: Option<Vec<String>>, timeout_secs: Option<u64> },
}

/// Server command representation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerCommand {
    pub id: String,
    pub spec: CommandSpec,
    pub created_at: Option<String>,
    pub run_as_root: Option<bool>,
}

/// Result object to post back to server for a command
#[derive(Debug, Serialize, Deserialize)]
struct CommandResult {
    pub id: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_secs: f64,
    pub success: bool,
}

/// SystemInfo structure used in payloads and locally.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SystemInfo {
    pub hostname: String,
    pub os_name: String,
    pub architecture: String,

    pub cpu_usage: f32,
    pub cpu_count: i32,
    pub cpu_brand: String,

    pub ram_total: i64,
    pub ram_used: i64,

    pub disk_total: i64,
    pub disk_free: i64,
    pub disk_health: String,

    pub network_throughput: i64,
    pub ping_latency: Option<f32>,

    pub network_interfaces: Option<String>,
    pub ip_address: Option<String>,

    // device_type / model are client-level metadata (may be empty)
    pub device_type: String,
    pub device_model: String,
}

impl SystemInfo {
    /// Blocking gather (spawn_blocking should be used by async callers).
    pub fn gather_blocking() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let hostname = sys.host_name().unwrap_or_else(|| "unknown".to_string());
        let os_name = sys.long_os_version().unwrap_or_else(|| "unknown".to_string());
        let kernel_version = sys.kernel_version().unwrap_or_else(|| "unknown".to_string());
        let architecture = std::env::consts::ARCH.to_string();

        let cpus = sys.cpus();
        let cpu_count = cpus.len() as i32;
        let cpu_brand = cpus.get(0).map(|c| c.brand().to_string()).unwrap_or_default();
        let cpu_usage = if cpu_count == 0 { 0.0 } else { cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpu_count as f32 };
        else {
            let sum: f32 = sys.processors().iter().map(|c| c.cpu_usage()).sum();
            sum / cpu_count as f32
        };

        let ram_total = sys.total_memory() as i64;
        let ram_used = sys.used_memory() as i64;

        let mut disk_total: i64 = 0;
        let mut disk_free: i64 = 0;
        for disk in sys.disks() {
            disk_total += disk.total_space() as i64;
            disk_free += disk.available_space() as i64;
        }

        let ip_address = local_ip().ok().map(|ip| ip.to_string());

        Self {
            hostname,
            os_name,
            architecture,
            cpu_usage,
            cpu_count,
            cpu_brand,
            ram_total,
            ram_used,
            disk_total,
            disk_free,
            disk_health: "".into(),
            network_throughput: 0,
            ping_latency: None,
            network_interfaces: None,
            ip_address,
            device_type: "".into(),
            device_model: "".into(),
        }
    }

    /// Synchronous convenience helper
    pub fn get_system_info() -> Self {
        Self::gather_blocking()
    }
}

/// Async service providing a cached SystemInfo snapshot
#[derive(Clone)]
pub struct SystemInfoService {
    cache: Arc<tokio::sync::RwLock<Option<SystemInfo>>>,
    last: Arc<tokio::sync::RwLock<Option<std::time::Instant>>>,
}

impl Default for SystemInfoService {
    fn default() -> Self {
        Self {
            cache: Arc::new(tokio::sync::RwLock::new(None)),
            last: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }
}

impl SystemInfoService {
    pub async fn get_system_info_async(&self) -> Result<SystemInfo> {
        let refresh_secs = get_system_info_refresh_secs();

        // Fast path
        {
            let last = self.last.read().await;
            let cache = self.cache.read().await;
            if let (Some(ts), Some(si)) = (*last, &*cache) {
                if ts.elapsed() < Duration::from_secs(refresh_secs) {
                    return Ok(si.clone());
                }
            }
        }

        // Upgrade to write lock
        let mut last = self.last.write().await;
        let mut cache = self.cache.write().await;

        if let (Some(ts), Some(si)) = (*last, &*cache) {
            if ts.elapsed() < Duration::from_secs(refresh_secs) {
                return Ok(si.clone());
            }
        }

        let info = tokio::task::spawn_blocking(move || SystemInfo::gather_blocking())
            .await
            .context("spawn_blocking failed")?;

        *cache = Some(info.clone());
        *last = Some(std::time::Instant::now());
        Ok(info)
    }
}

/// Convenience free function
pub fn get_system_info() -> SystemInfo {
    SystemInfo::gather_blocking()
}

/// Local device helpers
fn get_local_device_id() -> Option<String> {
    fs::read_to_string(DEVICE_ID_FILE).ok().map(|s| s.trim().to_string())
}

fn write_local_device_id(device_id: &str) -> Result<()> {
    fs::write(DEVICE_ID_FILE, device_id).context("Failed to write local device_id")
}

fn get_device_info_basic() -> (String, String) {
    let si = SystemInfo::gather_blocking();
    let device_type = si.device_type.clone();
    let device_model = si.device_model.clone();
    (device_type, device_model)
}

/// Register device (returns pending_id if created)
async fn register_device(
    client: &Client,
    server_url: &str,
    device_type: &str,
    device_model: &str,
) -> Result<String> {
    let sys_info = SystemInfo::gather_blocking();

    let payload = json!({
        "system_info": sys_info,
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
        .context("Error sending registration")?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if !status.is_success() {
        anyhow::bail!("Registration failed {}: {}", status, body);
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&body).context("Server returned invalid JSON")?;

    if let Some(pid) = parsed.get("pending_id").and_then(|v| v.as_str()) {
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
    let sys_info = SystemInfo::gather_blocking();

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
    let sys_info = SystemInfo::gather_blocking();

    let payload = json!({
        "device_id": device_id,
        "system_info": sys_info,
        "device_type": device_type,
        "device_model": device_model
    });

    match client.post(format!("{}/api/devices/heartbeat", server_url)).json(&payload).send().await {
        Ok(r) => {
            if !r.status().is_success() {
                log::warn!("Heartbeat request returned non-OK: {}", r.status());
                return false;
            }
            match r.json::<serde_json::Value>().await {
                Ok(v) => {
                    if let Some(sec) = v.get("config")
                        .and_then(|c| c.get("system_info_refresh_secs"))
                        .and_then(|s| s.as_u64()) {
                        set_system_info_refresh_secs(sec);
                    }
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

async fn poll_for_commands_once(client: &Client, server_url: &str, device_id: &str) -> Result<Vec<ServerCommand>> {
    let resp = client.get(format!("{}/api/devices/{}/commands/poll", server_url, device_id)).send().await.context("Failed to poll commands")?;
    if !resp.status().is_success() {
        anyhow::bail!("Command poll failed: {}", resp.status());
    }
    let commands: Vec<ServerCommand> = resp.json().await.context("Invalid command JSON")?;
    Ok(commands)
}

async fn execute_command_and_collect_result(cmd: &ServerCommand) -> CommandResult {
    let start = std::time::Instant::now();
    let timeout_secs = match &cmd.spec {
        CommandSpec::Shell { timeout_secs, .. } => timeout_secs.unwrap_or(COMMAND_DEFAULT_TIMEOUT_SECS),
        CommandSpec::Script { timeout_secs, .. } => timeout_secs.unwrap_or(COMMAND_DEFAULT_TIMEOUT_SECS),
    };

    let spec_clone = cmd.spec.clone();
    let id_clone = cmd.id.clone();

    let run = tokio::task::spawn_blocking(move || {
        match spec_clone {
            CommandSpec::Shell { command, .. } => {
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
                let script_path = {
                    #[cfg(any(unix, target_os = "macos"))]
                    { std::path::PathBuf::from(format!("{}/{}", SCRIPTS_DIR, name)) }
                    #[cfg(windows)]
                    { std::path::PathBuf::from(format!("{}\\{}", SCRIPTS_DIR, name)) }
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

    let output_res = timeout(Duration::from_secs(timeout_secs), run).await;
    let duration = start.elapsed();

    match output_res {
        Ok(join_res) => match join_res {
            Ok(Ok(os_output)) => {
                let stdout = String::from_utf8_lossy(&os_output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&os_output.stderr).to_string();
                let code = os_output.status.code().unwrap_or(-1);
                CommandResult { id: id_clone, exit_code: code, stdout, stderr, duration_secs: duration.as_secs_f64(), success: os_output.status.success() }
            }
            Ok(Err(err_str)) => CommandResult { id: id_clone, exit_code: -1, stdout: "".into(), stderr: format!("spawn error: {}", err_str), duration_secs: duration.as_secs_f64(), success: false },
            Err(join_err) => CommandResult { id: id_clone, exit_code: -1, stdout: "".into(), stderr: format!("join error: {:?}", join_err), duration_secs: duration.as_secs_f64(), success: false },
        },
        Err(_) => CommandResult { id: id_clone, exit_code: -1, stdout: "".into(), stderr: format!("command timed out after {}s", timeout_secs), duration_secs: duration.as_secs_f64(), success: false },
    }
}

async fn post_command_result(client: &Client, server_url: &str, result: &CommandResult) -> Result<()> {
    let resp = client.post(format!("{}/api/commands/{}/result", server_url, result.id)).json(result).send().await.context("Failed to post command result")?;
    if !resp.status().is_success() {
        anyhow::bail!("Server rejected result: {}", resp.status());
    }
    Ok(())
}

/// `running_flag` is Option<Arc<AtomicBool>> so it is Send + 'static for tokio::spawn.
/// If provided, the loops will stop when the flag is set to false.
async fn command_poll_loop(client: Client, server_url: String, device_id: String, running_flag: Option<Arc<AtomicBool>>) {
    loop {
        if let Some(ref flag) = running_flag {
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
                    let r = execute_command_and_collect_result(&cmd).await;
                    if let Err(e) = post_command_result(&client, &server_url, &r).await {
                        log::warn!("Failed to post command result for {}: {}", r.id, e);
                    }
                }
            }
            Err(e) => log::warn!("Command poll failed: {}", e),
        }

        sleep(Duration::from_secs(DEFAULT_COMMAND_POLL_INTERVAL)).await;
    }
}

pub async fn run_adoption_and_update_loop(
    client: &Client,
    server_url: &str,
    running_flag: Option<Arc<AtomicBool>>
) -> Result<()> {
    let (device_type, device_model) = {
        // Best-effort synchronous shortcut for boot-time device_type/model
        let svc = SystemInfoService::default();
        let si = svc.get_system_info_async().await.unwrap_or_else(|_| SystemInfo::gather_blocking());
        (si.device_type.clone(), si.device_model.clone())
    };

    let mut device_id = get_local_device_id();

    if device_id.is_none() {
        loop {
            match register_device(client, server_url, &device_type, &device_model).await {
                Ok(id) => { log::info!("Received device_id from server: {}", id); write_local_device_id(&id)?; device_id = Some(id); break; }
                Err(e) => { log::warn!("No device_id yet. Retrying...: {}", e); sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL)).await; }
            }
        }
    }
    let device_id = device_id.unwrap();

    // keep checking heartbeat until adopted
    loop {
        if send_heartbeat(client, server_url, &device_id, &device_type, &device_model).await { break; }
        sleep(Duration::from_secs(ADOPTION_CHECK_INTERVAL)).await;
    }

    // start command poll loop in background if requested
    if true {
        let client_clone = client.clone();
        let server_clone = server_url.to_string();
        let device_clone = device_id.clone();
        let rf = running_flag.clone();
        tokio::spawn(async move {
            command_poll_loop(client_clone, server_clone, device_clone, rf).await;
        });
    }

    // Main update loop
    loop {
        if let Some(ref flag) = running_flag {
            if !flag.load(Ordering::SeqCst) {
                return Ok(());
            }
        }

        if let Err(e) = send_system_update(client, server_url, &device_id, &device_type, &device_model).await {
            log::warn!("system_update failed: {}", e);
        }
        sleep(Duration::from_secs(DEFAULT_SYSTEM_UPDATE_INTERVAL)).await;
    }
}

#[cfg(any(unix, target_os = "macos"))]
pub async fn run_unix_service() -> Result<()> {
    let client = Client::new();
    let server_url = crate::service::read_server_url().await?;
    run_adoption_and_update_loop(&client, &server_url, None).await
}

#[cfg(windows)]
pub async fn run_service() -> Result<()> {
    use windows_service::{service::{ServiceControl, ServiceControlHandlerResult}, service_control_handler};
    use std::sync::Arc;

    let running_flag = Arc::new(AtomicBool::new(true));
    let running_flag_clone = running_flag.clone();

    fn service_main(flag: Arc<AtomicBool>) -> Result<()> {
        let client = Client::new();
        let server_url = futures::executor::block_on(read_server_url())?;
        futures::executor::block_on(run_adoption_and_update_loop(&client, &server_url, Some(flag.clone())))
    }

    let flag_for_handler = running_flag.clone();
    let _status = service_control_handler::register("PatchPilot", move |control| {
        match control {
            ServiceControl::Stop => { flag_for_handler.store(false, Ordering::SeqCst); ServiceControlHandlerResult::NoError }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    })?;

    service_main(running_flag_clone)
}
