mod system_info;
mod self_update;
#[cfg(windows)]
mod windows_service;

use anyhow::Result;
use log::{info, error};
use simplelog::{Config, LevelFilter, SimpleLogger};
use std::thread;
use std::time::Duration;
use reqwest::blocking::Client;
use serde_json::json;
use std::process::Command;

#[cfg(not(windows))]
fn run_linux_client_loop() -> Result<()> {
    info!("Linux Patch Client starting...");

    // Linux client main loop
    let client = Client::new();
    let server_url = "http://127.0.0.1:8080";  // Replace with actual server URL

    // Retry mechanism for heartbeat and system update
    let mut retries = 3;

    loop {
        // Send heartbeat to check adoption status
        let hostname = get_hostname(); // Fetch the system's hostname
        let system_info = system_info::get_system_info()?;
        let response = client.post(format!("{}/api/devices/heartbeat", server_url))
            .json(&json!( {
                "client_id": hostname, // Use hostname as unique identifier
                "system_info": system_info // Add the system info in heartbeat
            }))
            .send();

        match response {
            Ok(resp) if resp.status().is_success() => {
                let status: serde_json::Value = resp.json()?;
                if status["adopted"].as_bool() == Some(true) {
                    info!("Client approved.");
                    break;
                } else {
                    info!("Client awaiting approval.");
                }
            },
            Err(e) => {
                error!("Error sending heartbeat: {:?}", e);
                retries -= 1;
                if retries == 0 {
                    break;
                }
            }
        }

        // Sleep before retry
        thread::sleep(Duration::from_secs(60));  // Retry every minute
    }

    Ok(())
}

// Fetch the hostname from the system
fn get_hostname() -> String {
    let output = Command::new("hostname")
        .output()
        .expect("Failed to execute command");
    String::from_utf8_lossy(&output.stdout).to_string().trim().to_string()
}

fn main() -> Result<()> {
    // Initialize logging
    SimpleLogger::init(LevelFilter::Info, Config::default())?;

    // Run the client loop for Linux
    run_linux_client_loop()?;

    Ok(())
}
