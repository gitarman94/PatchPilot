use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Stdio;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use tokio::time::timeout;

#[cfg(any(unix, target_os = "macos"))]
const SCRIPTS_DIR: &str = "/opt/patchpilot_client/scripts";
#[cfg(windows)]
const SCRIPTS_DIR: &str = "C:\\ProgramData\\PatchPilot\\scripts";

// Defaults
pub const COMMAND_POLL_INTERVAL_SECS: u64 = 5;
pub const COMMAND_EXEC_TIMEOUT_SECS: u64 = 300;
pub const COMMAND_LONGPOLL_TIMEOUT_SECS: u64 = 60;
pub const COMMAND_RETRY_BACKOFF_SECS: u64 = 5;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CommandSpec {
    #[serde(rename = "shell")]
    Shell {
        command: String,
        timeout_secs: Option<u64>,
    },

    #[serde(rename = "script")]
    Script {
        name: String,
        args: Option<Vec<String>>,
        timeout_secs: Option<u64>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerCommand {
    pub id: String,
    pub spec: CommandSpec,
    pub created_at: Option<String>,
    pub run_as_root: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResult {
    pub id: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_secs: f64,
    pub success: bool,
}

/// Poll the server once for new commands
pub async fn poll_for_commands_once(
    client: &Client,
    server_url: &str,
    device_id: &str,
) -> Result<()> {
    let resp = client
        .get(format!("{}/api/devices/{}/commands/poll", server_url, device_id))
        .send()
        .await?;

    if !resp.status().is_success() {
        log::warn!("Command poll rejected: {}", resp.status());
        return Ok(());
    }

    let commands: Vec<Value> = resp.json().await?;
    for cmd_item in commands {
        crate::command::execute_command_and_post_result(
            client.clone(),
            server_url.to_string(),
            device_id.to_string(),
            cmd_item,
        ).await;
    }

    Ok(())
}

/// Execute a ServerCommand and return the CommandResult
pub async fn execute_command_and_collect_result(cmd: &ServerCommand) -> CommandResult {
    let start = std::time::Instant::now();

    let timeout_secs = match &cmd.spec {
        CommandSpec::Shell { timeout_secs, .. } => timeout_secs.unwrap_or(COMMAND_EXEC_TIMEOUT_SECS),
        CommandSpec::Script { timeout_secs, .. } => timeout_secs.unwrap_or(COMMAND_EXEC_TIMEOUT_SECS),
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
                    c.output().map_err(|e| format!("failed spawn: {}", e))
                }
                #[cfg(windows)]
                {
                    let mut c = std::process::Command::new("powershell");
                    c.arg("-NoProfile")
                        .arg("-NonInteractive")
                        .arg("-Command")
                        .arg(command);
                    c.stdin(Stdio::null());
                    c.stdout(Stdio::piped());
                    c.stderr(Stdio::piped());
                    c.output().map_err(|e| format!("failed spawn: {}", e))
                }
            }

            CommandSpec::Script { name, args, .. } => {
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
                c.output().map_err(|e| format!("failed spawn: {}", e))
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

                CommandResult {
                    id: id_clone,
                    exit_code: code,
                    stdout,
                    stderr,
                    duration_secs: duration.as_secs_f64(),
                    success: os_output.status.success(),
                }
            }

            Ok(Err(err_str)) => CommandResult {
                id: id_clone,
                exit_code: -1,
                stdout: "".into(),
                stderr: format!("spawn error: {}", err_str),
                duration_secs: duration.as_secs_f64(),
                success: false,
            },

            Err(join_err) => CommandResult {
                id: id_clone,
                exit_code: -1,
                stdout: "".into(),
                stderr: format!("join error: {:?}", join_err),
                duration_secs: duration.as_secs_f64(),
                success: false,
            },
        },

        Err(_) => CommandResult {
            id: id_clone,
            exit_code: -1,
            stdout: "".into(),
            stderr: format!("command timed out after {}s", timeout_secs),
            duration_secs: duration.as_secs_f64(),
            success: false,
        },
    }
}

/// Post a CommandResult back to the server
pub async fn post_command_result(
    client: &Client,
    server_url: &str,
    result: &CommandResult,
) -> Result<()> {
    let url = format!("{}/api/commands/{}/result", server_url, result.id);

    let resp = client
        .post(&url)
        .json(result)
        .send()
        .await
        .context("Failed to post command result")?;

    if !resp.status().is_success() {
        anyhow::bail!("Server rejected result: {}", resp.status());
    }

    Ok(())
}

/// Execute a command and post the result
pub async fn execute_command_and_post_result(
    client: Client,
    server_url: String,
    device_id: String,
    cmd_json: Value,
) {
    let parsed: Result<ServerCommand> = serde_json::from_value(cmd_json).context("Invalid command JSON");
    let cmd = match parsed {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to parse ServerCommand: {}", e);
            return;
        }
    };

    let result = execute_command_and_collect_result(&cmd).await;

    if let Err(e) = post_command_result(&client, &server_url, &result).await {
        log::warn!("Failed to post command result: {}", e);
    }
}

/// Start the polling loop for commands
pub async fn start_command_polling(
    client: Client,
    server_url: String,
    device_id: String,
    running_flag: Option<Arc<AtomicBool>>,
) -> Result<()> {
    log::info!("Starting command polling for device {} at {}", device_id, server_url);

    // Kick off the long‑poll loop
    command_poll_loop(client, server_url, device_id, running_flag).await;

    Ok(())
}

/// Continuous command polling loop
pub async fn command_poll_loop(
    client: Client,
    server_url: String,
    device_id: String,
    running_flag: Option<Arc<AtomicBool>>,
) {
    log::info!("Starting command poll loop for device {}", device_id);

    loop {
        if let Some(ref flag) = running_flag {
            if !flag.load(Ordering::SeqCst) {
                log::info!("Command poll loop stopping due to shutdown flag");
                return;
            }
        }

        let url = format!("{}/api/devices/{}/commands/poll", server_url, device_id);

        let request_future = client.get(&url).send();

        match timeout(Duration::from_secs(COMMAND_LONGPOLL_TIMEOUT_SECS), request_future).await {
            Ok(Ok(resp)) => {
                if !resp.status().is_success() {
                    log::warn!("Command poll returned non-OK: {}", resp.status());
                    tokio::time::sleep(Duration::from_secs(COMMAND_RETRY_BACKOFF_SECS)).await;
                    continue;
                }

                match resp.json::<Value>().await {
                    Ok(val) => {
                        if let Some(arr) = val.as_array() {
                            if arr.is_empty() {
                                tokio::time::sleep(Duration::from_secs(COMMAND_POLL_INTERVAL_SECS)).await;
                                continue;
                            }

                            for item in arr {
                                let client_clone = client.clone();
                                let server_clone = server_url.clone();
                                let device_clone = device_id.clone();
                                let cmd_clone = item.clone();

                                tokio::spawn(async move {
                                    execute_command_and_post_result(client_clone, server_clone, device_clone, cmd_clone).await;
                                });
                            }
                        } else {
                            log::warn!("Unexpected response to command poll: {:?}", val);
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to parse command poll JSON: {}", e);
                        tokio::time::sleep(Duration::from_secs(COMMAND_RETRY_BACKOFF_SECS)).await;
                    }
                }
            }

            Ok(Err(e)) => {
                log::warn!("Command poll HTTP error: {}", e);
                tokio::time::sleep(Duration::from_secs(COMMAND_RETRY_BACKOFF_SECS)).await;
            }

            Err(_) => {
                // Long poll timeout, normal
                continue;
            }
        }
    }
}
