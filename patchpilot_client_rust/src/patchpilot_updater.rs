use std::{
    env,
    fs,
    process::{Command, exit},
    thread::sleep,
    time::Duration,
};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: patchpilot_updater <old_exe_path> <new_exe_path>");
        exit(1);
    }

    let old_path = &args[1];
    let new_path = &args[2];

    println!("[*] Waiting for main process to exit...");

    // Wait a moment to allow main process to exit and release the file lock
    sleep(Duration::from_secs(2));

    // Try multiple times in case OS still has file lock
    let mut retries = 5;
    while retries > 0 {
        match fs::rename(new_path, old_path) {
            Ok(_) => {
                println!("[*] Successfully replaced binary.");
                break;
            }
            Err(e) => {
                eprintln!("[!] Failed to replace binary: {}. Retrying...", e);
                retries -= 1;
                sleep(Duration::from_secs(1));
            }
        }
    }

    if retries == 0 {
        eprintln!("[!] Failed to replace binary after multiple attempts.");
        exit(1);
    }

    println!("[*] Restarting application...");

    let status = Command::new(old_path)
        .spawn()
        .expect("Failed to restart the application");

    println!("[✔] Update complete.");
    let _ = status;
}
