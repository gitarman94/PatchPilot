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

    loop {
        // Send heartbeat to check adoption status
        let system_info = system_info::get_system_info()?; // Fetch system info from system_info.rs
        let response = client.post(format!("{}/api/devices/heartbeat", server_url))
            .json(&json!({
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
            _ => error!("Failed to check adoption status."),
        }

        // Wait for the next heartbeat
        thread::sleep(Duration::from_secs(30)); // Heartbeat interval
    }

    // Report system info once adopted
    loop {
        info!("Sending system update...");

        let system_info = system_info::get_system_info()?; // Fetch system info from system_info.rs
        let response = client.post(format!("{}/api/devices/update_status", server_url))
            .json(&json!({
                "client_id": "unique-client-id",
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
    // Init logger
    SimpleLogger::init(LevelFilter::Info, Config::default()).unwrap();

    info!("Rust Patch Client starting...");

    // Spawn self-update thread
    thread::spawn(|| {
        loop {
            if let Err(e) = self_update::check_and_update() {
                error!("Self-update failed: {:?}", e);
            }
            thread::sleep(Duration::from_secs(3600)); // hourly
        }
    });

    // Run platform-specific main loop
    #[cfg(windows)]
    {
        windows_service::run_service()?; // Windows service management
    }

    #[cfg(not(windows))]
    {
        run_linux_client_loop()?; // Linux client loop
    }

    Ok(())
}


