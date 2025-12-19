use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

#[cfg(unix)]
use libc;

#[cfg(any(unix, target_os = "macos"))]
const SCRIPTS_DIR: &str = "/opt/patchpilot_client/scripts";
#[cfg(windows)]
const SCRIPTS_DIR: &str = "C:\\ProgramData\\PatchPilot\\scripts";

/// How often the agent checks for new commands after an empty poll
pub const COMMAND_POLL_INTERVAL_SECS: u64 = 5;

/// How long a single command is allowed to run
pub const COMMAND_EXEC_TIMEOUT_SECS: u64 = 300;

/// How long a poll request may long-poll before timing out
pub const COMMAND_LONGPOLL_TIMEOUT_SECS: u64 = 60;

/// How long to back off on HTTP errors
pub const COMMAND_RETRY_BACKOFF_SECS: u64 = 5;

/// A structured representation of what to run
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

/// A command received from the server
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerCommand {
    pub id: String,
    pub spec: CommandSpec,
    pub created_at: Option<String>,
    pub run_as_root: Option<bool>,
}

/// A summary of execution for posting back
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResult {
    pub id: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_secs: f64,
    pub success: bool,
}

/// Check whether we are running with root/admin privileges
#[cfg(unix)]
fn check_root(required: bool) -> Result<()> {
    if required && unsafe { libc::geteuid() } != 0 {
        anyhow::bail!("action requires root privileges but none present");
    }
    Ok(())
}

#[cfg(windows)]
fn check_admin(_required: bool) -> Result<()> {
    Ok(())
}

/// Poll the server once for new commands
pub async fn poll_for_commands_once(
    client: &Client,
    server_url: &str,
    device_id: &str,
) -> Result<Vec<ServerCommand>> {
    log::debug!("Polling server for commands for device {}", device_id);

    let resp = client
        .get(format!("{}/api/devices/{}/commands/poll", server_url, device_id))
        .send()
        .await?;

    if !resp.status().is_success() {
        log::warn!("Command poll rejected: {}", resp.status());
        return Ok(vec![]);
    }

    let cmds: Vec<Value> = resp.json().await?;
    let mut out = Vec::with_capacity(cmds.len());
    for raw in cmds {
        match serde_json::from_value::<ServerCommand>(raw) {
            Ok(cmd) => out.push(cmd),
            Err(e) => log::warn!("Invalid command JSON: {}", e),
        }
    }
    Ok(out)
}

/// Execute a command via the engine in `command.rs`
pub async fn execute_action(
    client: Client,
    server_url: String,
    device_id: String,
    cmd: ServerCommand,
) {
    if let Some(run_as_root) = cmd.run_as_root {
        #[cfg(unix)]
        if let Err(e) = check_root(run_as_root) {
            log::warn!("Skipping root-required action {}: {}", cmd.id, e);
            return;
        }
        #[cfg(windows)]
        let _ = check_admin(run_as_root);
    }

    // Delegate actual execution to engine
    let exec_result = crate::command::execute_command(cmd.clone()).await;

    match exec_result {
        Ok(execution) => {
            // Convert ExecutionResult -> CommandResult
            let result = CommandResult {
                id: execution.id.clone(),
                exit_code: execution.exit_code,
                stdout: execution.stdout.clone(),
                stderr: execution.stderr.clone(),
                duration_secs: 0.0,
                success: execution.exit_code == 0,
            };

            if let Err(e) = crate::command::post_command_result(
                &client,
                &server_url,
                &execution.id,
                &result,
            ).await
            {
                log::warn!("Failed to post result for {}: {}", cmd.id, e);
            }
        }
        Err(e) => {
            log::warn!("Execution failed for {}: {:?}", cmd.id, e);
        }
    }
}

/// Action loop: poll continuously and dispatch
pub async fn action_loop(
    client: Client,
    server_url: String,
    device_id: String,
    running_flag: Option<Arc<AtomicBool>>,
) -> Result<()> {
    loop {
        if let Some(flag) = &running_flag {
            if !flag.load(Ordering::SeqCst) {
                log::info!("Action loop stopping due to shutdown flag");
                break;
            }
        }

        let commands = poll_for_commands_once(&client, &server_url, &device_id).await?;
        for cmd in commands {
            let c = client.clone();
            let s = server_url.clone();
            let d = device_id.clone();
            tokio::spawn(async move {
                execute_action(c, s, d, cmd).await;
            });
        }

        tokio::time::sleep(std::time::Duration::from_secs(COMMAND_POLL_INTERVAL_SECS)).await;
    }

    Ok(())
}
