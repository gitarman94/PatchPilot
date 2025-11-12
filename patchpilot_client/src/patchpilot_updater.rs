use std::process::Command;
use std::fs;
use std::{thread, time::Duration};

/// Simple console logger
fn log(level: &str, message: &str) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    println!("{} [{}] {}", timestamp, level, message);
}

/// Entry point for the updater binary.
/// Replaces the running binary with a new one and restarts it.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        log("ERROR", "Usage: patchpilot_updater <old_exe_path> <new_exe_path>");
        std::process::exit(1);
    }

    let old_path = &args[1];
    let new_path = &args[2];

    log("INFO", "PatchPilot updater started.");
    log("INFO", "Waiting 2 seconds for main process to exit...");
    thread::sleep(Duration::from_secs(2));

    const MAX_RETRIES: u32 = 5;
    let mut retries = MAX_RETRIES;

    // Try replacing binary with retries
    while retries > 0 {
        match fs::rename(new_path, old_path) {
            Ok(_) => {
                log("INFO", "✔ Successfully replaced binary.");
                break;
            }
            Err(e) => {
                retries -= 1;
                log("WARN", &format!(
                    "Failed to replace binary ({:?}). Retries left: {}...",
                    e, retries
                ));
                thread::sleep(Duration::from_secs(1));
            }
        }
    }

    if retries == 0 {
        log("ERROR", "✖ Failed to replace binary after max retries.");
        std::process::exit(1);
    }

    // Restart the updated binary
    log("INFO", "Attempting to restart updated application...");
    match Command::new(old_path).spawn() {
        Ok(_) => log("INFO", "✔ Update complete. Application restarted successfully."),
        Err(e) => {
            log("ERROR", &format!("✖ Failed to restart application: {:?}", e));
            std::process::exit(1);
        }
    }

    log("INFO", "PatchPilot update process completed successfully.");
}
