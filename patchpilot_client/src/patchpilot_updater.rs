use std::{
    env,
    fs,
    process::{Command, exit},
    thread::sleep,
    time::Duration,
};
use log::{info, warn, error};
use simplelog::{SimpleLogger, Config, LevelFilter};

fn main() {
    // Initialize logger
    SimpleLogger::init(LevelFilter::Info, Config::default())
        .expect("Failed to initialize logger");

    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: patchpilot_updater <old_exe_path> <new_exe_path>");
        exit(1);
    }

    let old_path = &args[1];
    let new_path = &args[2];

    info!("[*] PatchPilot updater started.");
    info!("[*] Waiting 2 seconds for main process to exit...");
    sleep(Duration::from_secs(2));

    const MAX_RETRIES: u8 = 5;
    let mut retries = MAX_RETRIES;

    while retries > 0 {
        match fs::rename(new_path, old_path) {
            Ok(_) => {
                info!("[✔] Successfully replaced binary at '{}'.", old_path);
                break;
            }
            Err(e) => {
                retries -= 1;
                warn!(
                    "[!] Failed to replace binary ({}). Retries left: {}. Retrying in 1s...",
                    e, retries
                );
                sleep(Duration::from_secs(1));
            }
        }
    }

    if retries == 0 {
        error!(
            "[✖] Failed to replace binary '{}' after {} attempts. Aborting.",
            old_path, MAX_RETRIES
        );
        exit(1);
    }

    info!("[*] Attempting to restart application: '{}'", old_path);

    match Command::new(old_path).spawn() {
        Ok(_child) => {
            info!("[✔] Update complete. Application restarted successfully.");
        }
        Err(e) => {
            error!("[✖] Failed to restart application '{}': {}", old_path, e);
            exit(1);
        }
    }
}
