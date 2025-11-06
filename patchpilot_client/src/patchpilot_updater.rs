use std::process::Command;
use std::fs;
use std::{thread, time::Duration};

/// Function to log messages
fn log(level: &str, message: &str) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    println!("{} [{}] {}", timestamp, level, message);
}

/// Main function to handle the update logic
fn main() {
    // Check if the correct number of arguments is passed
    if std::env::args().len() != 3 {
        log("ERROR", "Usage: patchpilot_updater <old_exe_path> <new_exe_path>");
        std::process::exit(1);
    }

    let args: Vec<String> = std::env::args().collect();
    let old_path = &args[1];
    let new_path = &args[2];

    // Log the start of the update process
    log("INFO", "[*] PatchPilot updater started.");
    log("INFO", "[*] Waiting 2 seconds for the main process to exit...");
    thread::sleep(Duration::new(2, 0));

    // Maximum retries and current retry count
    const MAX_RETRIES: u32 = 5;
    let mut retries = MAX_RETRIES;

    // Attempt to replace the old binary with the new one, with retries
    while retries > 0 {
        match fs::rename(new_path, old_path) {
            Ok(_) => {
                log("INFO", "[✔] Successfully replaced binary.");
                break;
            }
            Err(_) => {
                retries -= 1;
                log(
                    "WARN",
                    &format!("[!] Failed to replace binary. Retries left: {}. Retrying in 1 second...", retries),
                );
                thread::sleep(Duration::new(1, 0));
            }
        }
    }

    // Check if the replacement was successful
    if retries == 0 {
        log("ERROR", "[✖] Failed to replace binary after max retries.");
        std::process::exit(1);
    }

    // Attempt to restart the application
    log("INFO", "[*] Attempting to restart application.");
    if Command::new(old_path).spawn().is_ok() {
        log("INFO", "[✔] Update complete. Application restarted successfully.");
    } else {
        log("ERROR", "[✖] Failed to restart application.");
        std::process::exit(1);
    }

    log("INFO", "[*] PatchPilot update process completed successfully.");
}
