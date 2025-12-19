use anyhow::Result;
use reqwest::Client;
use tokio::task;
use std::process::Command;
use crate::action::{CommandSpec, ServerCommand, CommandResult};

/// A structured execution result
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub id: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Run a single command spec
pub async fn execute_command(cmd: ServerCommand) -> Result<ExecutionResult> {
    let id = cmd.id.clone();

    // Prepare the command text
    let (program, args): (String, Vec<String>) = match cmd.spec {
        CommandSpec::Shell { command, .. } => {
            #[cfg(windows)]
            {
                ("cmd".into(), vec!["/C".into(), command])
            }
            #[cfg(not(windows))]
            {
                ("sh".into(), vec!["-c".into(), command])
            }
        }
        CommandSpec::Script { name, args, .. } => {
            let mut all_args = Vec::new();
            all_args.push(name.clone());
            if let Some(extra) = args {
                all_args.extend(extra);
            }
            (all_args.remove(0), all_args)
        }
    };

    let run = task::spawn_blocking(move || {
        Command::new(program)
            .args(&args)
            .output()
            .map_err(|e| format!("failed spawn: {}", e))
    })
    .await?;

    let output = match run {
        Ok(o) => o,
        Err(e) => {
            return Err(anyhow::anyhow!("Execution failed: {}", e));
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    Ok(ExecutionResult {
        id,
        stdout,
        stderr,
        exit_code: code,
    })
}

/// Post execution result to server
pub async fn post_command_result(
    client: &Client,
    server_url: &str,
    cmd_id: &str,
    result: &CommandResult,
) -> Result<()> {
    let url = format!("{}/api/commands/{}/result", server_url, cmd_id);

    // Explicit type annotation to satisfy Rust
    let resp: reqwest::Response = client.post(&url).json(result).send().await?;

    if !resp.status().is_success() {
        log::warn!("Server rejected command result {}: {}", cmd_id, resp.status());
    }

    Ok(())
}
