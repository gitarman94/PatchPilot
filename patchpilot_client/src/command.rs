use anyhow::Result;
use reqwest::Client;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::time::{timeout, Duration, sleep};
use tokio::task;

pub const COMMAND_LONGPOLL_TIMEOUT_SECS: u64 = 60;
pub const COMMAND_RETRY_BACKOFF_SECS: u64 = 5;

/// Execute a single command (shell/script)
pub async fn execute_command_and_post_result(
    client: Client,
    server_url: String,
    device_id: String,
    cmd_item: Value,
) {
    let cmd_id: String = cmd_item
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let cmd_string: String = cmd_item
        .get("exec")
        .and_then(|v| v.as_str())
        .or_else(|| cmd_item.get("script").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "".to_string());

    if cmd_string.is_empty() {
        let _ = post_command_result(
            &client,
            &server_url,
            &device_id,
            &cmd_id,
            json!({
                "status": "error",
                "reason": "missing exec/script field"
            }),
        )
        .await;
        return;
    }

    let kind: String = cmd_item
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("exec")
        .to_string();

    let cmd_for_exec = cmd_string.clone();

    let run = task::spawn_blocking(move || {
        #[cfg(windows)]
        let out = std::process::Command::new("cmd")
            .args(&["/C", &cmd_for_exec])
            .output();

        #[cfg(not(windows))]
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd_for_exec)
            .output();

        match out {
            Ok(o) => (
                true,
                String::from_utf8_lossy(&o.stdout).to_string(),
                String::from_utf8_lossy(&o.stderr).to_string(),
                o.status.code().unwrap_or(-1),
            ),
            Err(e) => (
                false,
                String::new(),
                format!("Failed to start process: {}", e),
                -1,
            ),
        }
    })
    .await;

    match run {
        Ok((ok, stdout, stderr, exit_code)) => {
            let payload = json!({
                "status": if ok { "ok" } else { "error" },
                "kind": kind,
                "stdout": stdout,
                "stderr": stderr,
                "exit_code": exit_code
            });

            let _ = post_command_result(
                &client,
                &server_url,
                &device_id,
                &cmd_id,
                payload,
            )
            .await;
        }
        Err(e) => {
            log::error!("Command thread panicked: {}", e);
            let _ = post_command_result(
                &client,
                &server_url,
                &device_id,
                &cmd_id,
                json!({
                    "status": "error",
                    "reason": format!("panic: {}", e)
                }),
            )
            .await;
        }
    }
}

/// Post command result to server
pub async fn post_command_result(
    client: &Client,
    server_url: &str,
    device_id: &str,
    cmd_id: &str,
    payload: Value,
) -> Result<()> {
    let url = format!(
        "{}/api/devices/{}/commands/{}/result",
        server_url, device_id, cmd_id
    );

    let resp = client.post(&url).json(&payload).send().await?;

    if !resp.status().is_success() {
        log::warn!(
            "Server rejected command result {}: {}",
            cmd_id,
            resp.status()
        );
    }

    Ok(())
}

/// Long-poll loop
pub async fn command_poll_loop(
    client: Client,
    server_url: String,
    device_id: String,
    running_flag: Option<Arc<AtomicBool>>,
) {
    loop {
        if let Some(flag) = &running_flag {
            if !flag.load(Ordering::SeqCst) {
                break;
            }
        }

        let req = client
            .get(format!(
                "{}/api/devices/{}/commands/poll",
                server_url, device_id
            ))
            .send();

        match timeout(Duration::from_secs(COMMAND_LONGPOLL_TIMEOUT_SECS), req).await {
            Ok(Ok(resp)) => {
                if !resp.status().is_success() {
                    sleep(Duration::from_secs(COMMAND_RETRY_BACKOFF_SECS)).await;
                    continue;
                }

                if let Ok(val) = resp.json::<Value>().await {
                    if let Some(arr) = val.as_array() {
                        for cmd_item in arr {
                            let c = client.clone();
                            let s = server_url.clone();
                            let d = device_id.clone();
                            let ci = cmd_item.clone();

                            tokio::spawn(async move {
                                execute_command_and_post_result(c, s, d, ci).await;
                            });
                        }
                    }
                }
            }
            _ => {
                sleep(Duration::from_secs(COMMAND_RETRY_BACKOFF_SECS)).await;
            }
        }
    }
}
