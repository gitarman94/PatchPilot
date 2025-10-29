use std::{
    env,
    fs,
    process::{Command, exit},
    thread::sleep,
    time::Duration,
};
use log::{info, error, warn};  // Import logging macros

fn main() {
    // Initialize logger
    simplelog::SimpleLogger::init(simplelog::LevelFilter::Info, simplelog::Config::default()).unwrap();

    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: patchpilot_updater <old_exe_path> <new_exe_path>");
        exit(1);
    }

    let old_path = &args[1];
    let new_path = &args[2];

    info!("[*] Waiting for main process to exit...");

    sleep(Duration::from_secs(2));

    let mut retries = 5;
    while retries > 0 {
        match fs::rename(new_path, old_path) {
            Ok(_) => {
                info!("[*] Successfully replaced binary.");
                break;
            }
            Err(e) => {
                warn!("[!] Failed to replace binary: {}. Retrying...", e);
                retries -= 1;
                sleep(Duration::from_secs(1));
            }
        }
    }

    if retries == 0 {
        error!("[!] Failed to replace binary after multiple attempts.");
        exit(1);
    }

    info!("[*] Restarting application...");

    match Command::new(old_path)
        .spawn()
    {
        Ok(status) => {
            let _ = status;  // Optionally handle the process status
            info!("[✔] Update complete. Application restarted.");
        },
        Err(e) => {
            error!("[!] Failed to restart the application: {}", e);
            exit(1);
        }
    }
}
