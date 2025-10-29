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
        info!("Sending heartbeat to check adoption status...");
        let system_info = system_info::get_system_info()?; // Fetch system info from system_info.rs
        let response = client.post(format!("{}/api/devices/heartbeat", server_url))
            .json(&json!( {
                "client_id": "unique-client-id", // Use unique client ID here
                "system_info": system_info // Add the actual system info
            }))
            .send();

        match response {
            Ok(resp) if resp.status().is_success() => {
                let status: serde_json::Value = resp.json()?;
                if status["adopted"].as_bool() == Some(true) {
                    info!("Client approved. Starting system report loop...");
                    break; // Proceed to normal reporting after adoption
                } else {
                    info!("Waiting for approval...");
                }
            },
            Err(e) => {
                error!("Error sending heartbeat: {:?}", e);
                retries -= 1;
                if retries == 0 {
                    error!("Failed to check adoption status after multiple attempts.");
                    return Err(anyhow::anyhow!("Adoption check failed")).into();
                }
            },
            // Adding a wildcard match for any Ok response
            Ok(_) => {
                error!("Unexpected response type received");
                retries -= 1;
                if retries == 0 {
                    error!("Failed to check adoption status after multiple attempts.");
                    return Err(anyhow::anyhow!("Unexpected response type")).into();
                }
            }
        }

        // Wait for the next heartbeat
        thread::sleep(Duration::from_secs(30)); // Heartbeat interval
    }

    // Report system info once adopted
    loop {
        info!("Sending system update...");

        let system_info = system_info::get_system_info()?; // Fetch system info from system_info.rs
        let response = client.post(format!("{}/api/devices/update_status", server_url))
            .json(&json!( {
                "client_id": "unique-client-id", // Replace with actual unique client ID
                "status": "active", // Customize status if needed
                "system_info": system_info // Send system info here
            }))
            .send();

        if let Err(e) = response {
            error!("Failed to send system info: {:?}", e);
        }

        // Wait before sending the next update
        thread::sleep(Duration::from_secs(600)); // Regular update interval
    }
}

fn main() -> Result<()> {
    // Initialize logger
    SimpleLogger::init(LevelFilter::Info, Config::default()).unwrap();

    info!("Rust Patch Client starting...");

    // Spawn self-update thread
    thread::spawn(|| {
        loop {
            if let Err(e) = self_update::check_and_update() {
                error!("Self-update failed: {:?}", e);
            }
            thread::sleep(Duration::from_secs(3600)); // hourly self-update check
        }
    });

    // Run platform-specific main loop
    #[cfg(windows)]
    {
        if let Err(e) = windows_service::run_service() {
            error!("Failed to run Windows service: {:?}", e);
        }
    }

    #[cfg(not(windows))] // Linux or other non-Windows systems
    {
        if let Err(e) = run_linux_client_loop() {
            error!("Linux client loop failed: {:?}", e);
        }
    }

    Ok(())
}
