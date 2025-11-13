use std::{fs, process::Command, thread, time::Duration};
use log::{info, warn, error};
use flexi_logger::{Logger, Duplicate, Age, Cleanup};

/// Initialize logger (same as main.rs)
fn init_logger() {
    Logger::try_with_str("info")
        .unwrap()
        .log_to_file()
        .directory("logs")
        .duplicate_to_stderr(Duplicate::Info)
        .rotate(Age::Day, Cleanup::KeepLogFiles(7))
        .start()
        .unwrap_or_else(|e| panic!("Logger initialization failed: {}", e));
}

fn main() {
    // Initialize logger
    init_logger();
    info!("PatchPilot updater started.");

    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        error!("Usage: patchpilot_updater <old_exe_path> <new_exe_path>");
        std::process::exit(1);
    }

    let old_path = &args[1];
    let new_path = &args[2];

    info!("Waiting 2 seconds for main process to exit...");
    thread::sleep(Duration::from_secs(2));

    const MAX_RETRIES: u32 = 5;
    let mut retries = MAX_RETRIES;

    while retries > 0 {
        match fs::rename(new_path, old_path) {
            Ok(_) => {
                info!("✔ Successfully replaced binary.");
                break;
            }
            Err(e) => {
                retries -= 1;
                warn!("Failed to replace binary ({:?}). Retries left: {}...", e, retries);
                thread::sleep(Duration::from_secs(1));
            }
        }
    }

    if retries == 0 {
        error!("✖ Failed to replace binary after max retries.");
        std::process::exit(1);
    }

    // Restart the updated binary
    info!("Attempting to restart updated application...");
    match Command::new(old_path).spawn() {
        Ok(_) => info!("✔ Update complete. Application restarted successfully."),
        Err(e) => {
            error!("✖ Failed to restart application: {:?}", e);
            std::process::exit(1);
        }
    }

    info!("PatchPilot update process completed successfully.");
}
