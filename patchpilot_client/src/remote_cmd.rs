use serde::{Deserialize, Serialize};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{engine::general_purpose, Engine as _};
use std::path::Path;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tokio::sync::Semaphore;
use std::sync::Arc;

type HmacSha256 = Hmac<Sha256>;

/// Command shape we expect from server in heartbeat JSON
#[derive(Debug, Clone, serde::Serialize)]
pub struct RemoteCommand {
    pub id: String,               // server-generated id
    pub kind: String,             // "script" or "exec" (prefer "script")
    pub name: String,             // script filename (for kind=script) or binary (for exec)
    pub args: Option<Vec<String>>,
    pub timeout_secs: Option<u64>,
    pub signature: String,        // base64(hmac_sha256(secret, payload))
    // optional metadata fields...
}

/// Result we POST back to server for each command
#[derive(serde::Serialize, Debug)]
pub struct CommandResult {
    pub id: String,
    pub status: String,   // "ok" | "failed" | "rejected" | "timeout"
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub started_at: String,
    pub finished_at: String,
}

/// verify HMAC signature using shared secret bytes
pub fn verify_signature(cmd: &RemoteCommand, canonical_payload: &str, secret: &[u8]) -> bool {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC can take key");
    mac.update(canonical_payload.as_bytes());
    let expected = mac.finalize().into_bytes();
    match general_purpose::STANDARD.decode(&cmd.signature) {
        Ok(got) => got == expected.as_slice(),
        Err(_) => false,
    }
}

/// Allowed scripts directory (whitelist by location)
pub fn allowed_script_path(script_name: &str) -> Option<std::path::PathBuf> {
    // change paths per OS
    #[cfg(any(unix, target_os = "macos"))]
    let base = "/opt/patchpilot_client/scripts";
    #[cfg(windows)]
    let base = "C:\\ProgramData\\PatchPilot\\scripts";

    let p = Path::new(base).join(script_name);
    if p.exists() && p.is_file() {
        Some(p)
    } else {
        None
    }
}

/// Execution semaphore (limit concurrent commands)
pub fn concurrency_semaphore() -> Arc<Semaphore> {
    Arc::new(Semaphore::new(2)) // default: 2 concurrent commands
}

/// Execute a command (script or exec) with timeout, capture stdout/stderr.
pub async fn execute_remote_command(
    cmd: RemoteCommand,
    client: reqwest::Client,
    server_url: String,
    device_id: String,
    auth_token: Option<String>,
    secret: Vec<u8>,
    sem: Arc<Semaphore>,
) -> anyhow::Result<()> {
    use chrono::Utc;

    // canonical_payload should be the same bytes server signed (choose canonicalization!)
    // For simplicity assume server used serde_json::to_string(&cmd_without_signature)
    // Client must reconstruct same canonical payload for verification.
    let mut cmd_for_sig = cmd.clone();
    let sig = cmd_for_sig.signature.clone();
    cmd_for_sig.signature = "".to_string(); // remove signature field
    let canonical = serde_json::to_string(&cmd_for_sig)?;

    if !verify_signature(&cmd, &canonical, &secret) {
        // reject
        let res = CommandResult {
            id: cmd.id.clone(),
            status: "rejected".into(),
            exit_code: None,
            stdout: "".into(),
            stderr: "signature verification failed".into(),
            started_at: Utc::now().to_rfc3339(),
            finished_at: Utc::now().to_rfc3339(),
        };
        post_result(&client, &server_url, &device_id, &cmd.id, &res, auth_token).await?;
        return Ok(());
    }

    // Acquire permit (bounded concurrency)
    let permit = sem.acquire().await.unwrap();

    let started = Utc::now();
    let timeout_dur = Duration::from_secs(cmd.timeout_secs.unwrap_or(60));

    // prepare actual command
    let mut process = match cmd.kind.as_str() {
        "script" => {
            // allow only scripts present in scripts dir
            let script_name = &cmd.name;
            if let Some(script_path) = allowed_script_path(script_name) {
                let mut c = Command::new(script_path);
                if let Some(args) = cmd.args.clone() {
                    c.args(args);
                }
                c
            } else {
                // not allowed -> report
                let res = CommandResult {
                    id: cmd.id.clone(),
                    status: "rejected".into(),
                    exit_code: None,
                    stdout: "".into(),
                    stderr: format!("script not allowed or missing: {}", cmd.name),
                    started_at: started.to_rfc3339(),
                    finished_at: Utc::now().to_rfc3339(),
                };
                drop(permit); // release
                post_result(&client, &server_url, &device_id, &cmd.id, &res, auth_token).await?;
                return Ok(());
            }
        }
        "exec" => {
            // exec only if the executable is explicitly permitted (add checks)
            // You should replace this check with a whitelist of allowed binaries.
            let safe_bins = ["patchpilot-helper", "some-other-safe-binary"];
            if !safe_bins.contains(&cmd.name.as_str()) {
                let res = CommandResult {
                    id: cmd.id.clone(),
                    status: "rejected".into(),
                    exit_code: None,
                    stdout: "".into(),
                    stderr: format!("exec '{}' not whitelisted", cmd.name),
                    started_at: started.to_rfc3339(),
                    finished_at: Utc::now().to_rfc3339(),
                };
                drop(permit);
                post_result(&client, &server_url, &device_id, &cmd.id, &res, auth_token).await?;
                return Ok(());
            }
            let mut c = Command::new(&cmd.name);
            if let Some(args) = cmd.args.clone() {
                c.args(args);
            }
            c
        }
        other => {
            let res = CommandResult {
                id: cmd.id.clone(),
                status: "rejected".into(),
                exit_code: None,
                stdout: "".into(),
                stderr: format!("unknown kind '{}'", other),
                started_at: started.to_rfc3339(),
                finished_at: Utc::now().to_rfc3339(),
            };
            drop(permit);
            post_result(&client, &server_url, &device_id, &cmd.id, &res, auth_token).await?;
            return Ok(());
        }
    };

    // spawn and await with timeout
    let run_future = async {
        let output = process.output().await;
        output
    };

    let result = timeout(timeout_dur, run_future).await;

    let finished = Utc::now();

    let cmd_result = match result {
        Ok(Ok(out)) => {
            CommandResult {
                id: cmd.id.clone(),
                status: "ok".into(),
                exit_code: out.status.code(),
                stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                stderr: String::from_utf8_lossy(&out.stderr).to_string(),
                started_at: started.to_rfc3339(),
                finished_at: finished.to_rfc3339(),
            }
        }
        Ok(Err(e)) => {
            CommandResult {
                id: cmd.id.clone(),
                status: "failed".into(),
                exit_code: None,
                stdout: "".into(),
                stderr: format!("failed to spawn/exec: {}", e),
                started_at: started.to_rfc3339(),
                finished_at: finished.to_rfc3339(),
            }
        }
        Err(_) => {
            CommandResult {
                id: cmd.id.clone(),
                status: "timeout".into(),
                exit_code: None,
                stdout: "".into(),
                stderr: "command timed out".into(),
                started_at: started.to_rfc3339(),
                finished_at: finished.to_rfc3339(),
            }
        }
    };

    // release permit
    drop(permit);

    // post result
    post_result(&client, &server_url, &device_id, &cmd.id, &cmd_result, auth_token).await?;

    Ok(())
}

/// Post command result helper
async fn post_result(
    client: &reqwest::Client,
    server_url: &str,
    device_id: &str,
    cmd_id: &str,
    res: &CommandResult,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let url = format!("{}/api/devices/{}/commands/{}/result", server_url.trim_end_matches('/'), device_id, cmd_id);
    let mut req = client.post(&url).json(res);
    if let Some(token) = auth_token {
        req = req.bearer_auth(token);
    }
    let _r = req.send().await?;
    Ok(())
}
