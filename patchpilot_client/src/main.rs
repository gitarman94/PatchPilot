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

// Main loop for Linux or non-Windows devices
#[cfg(not(windows))]
fn run_device_loop() -> Result<()> {
    info!("Patch Device starting...");

    let client = Client::new();
    let server_url = "http://127.0.0.1:8080"; // Replace with actual server URL
    let device_id = "unique-device-id";      // Replace with unique device ID

    loop {
        // Fetch system info
        let system_info = match system_info::get_system_info() {
            Ok(info) => info,
            Err(e) => {
                error!("Failed to get system info: {:?}", e);
                thread::sleep(Duration::from_secs(60));
                continue;
            }
        };

        // Send system info to server
        let response = client.post(format!("{}/api/devices/{}", server_url, device_id))
            .json(&system_info)
            .send();

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    info!("System info successfully sent to server.");
                } else if resp.status().as_u16() == 403 {
                    error!("Device not approved by server. Reporting skipped.");
                } else {
                    error!("Unexpected server response: {:?}", resp.status());
                }
            },
            Err(e) => {
                error!("Failed to send system info: {:?}", e);
            }
        }

        // Wait before next update
        thread::sleep(Duration::from_secs(600)); // 10-minute interval
    }
}

fn main() -> Result<()> {
    // Initialize logging
    SimpleLogger::init(LevelFilter::Info, Config::default()).unwrap();
    info!("Rust Patch Device starting...");

    // Start self-update thread
    thread::spawn(|| {
        loop {
            if let Err(e) = self_update::check_and_update() {
                error!("Self-update failed: {:?}", e);
            }
            thread::sleep(Duration::from_secs(3600)); // hourly self-update check
        }
    });

    // Platform-specific main loop
    #[cfg(windows)]
    {
        if let Err(e) = windows_service::run_service() {
            error!("Failed to run Windows service: {:?}", e);
        }
    }

    #[cfg(not(windows))]
    {
        if let Err(e) = run_device_loop() {
            error!("Device loop failed: {:?}", e);
        }
    }

    Ok(())
}


