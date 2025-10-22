mod self_update;
#[cfg(windows)]
mod windows_service;

use anyhow::Result;
use log::{info, error};
use simplelog::{Config, LevelFilter, SimpleLogger};
use std::thread;
use std::time::Duration;

#[cfg(not(windows))]
fn run_linux_client_loop() -> Result<()> {
    info!("Linux Patch Client starting...");

    // Your Linux client main loop here, for example:
    loop {
        // Placeholder: do Linux-specific patch client tasks
        info!("Linux client heartbeat...");

        // You can add your Linux update checks, system info reports, etc. here

        thread::sleep(Duration::from_secs(600)); // e.g., 10 minutes
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
        windows_service::run_service()?;
    }

    #[cfg(not(windows))]
    {
        run_linux_client_loop()?;
    }

    Ok(())
}
