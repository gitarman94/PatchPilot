use std::{fs, process::Command, thread, time::Duration};
use log::{info, warn, error};
use flexi_logger::{Logger, FileSpec, Duplicate, Criterion, Naming, Age, Cleanup};

#[cfg(windows)]
const LOG_DIR: &str = "C:\\ProgramData\\PatchPilot\\logs";
#[cfg(not(windows))]
const LOG_DIR: &str = "/opt/patchpilot_client/logs";

fn init_logger() {
    std::fs::create_dir_all(LOG_DIR).ok();

    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(FileSpec::default().directory(LOG_DIR))
        .duplicate_to_stderr(Duplicate::Info)
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Numbers,
            Cleanup::KeepLogFiles(7),
        )
        .start()
        .unwrap();
}

#[cfg(windows)]
fn launch_app(path: &str) -> std::io::Result<()> {
    Command::new("cmd")
        .arg("/C")
        .arg(format!("start \"\" \"{}\"", path))
        .spawn()?;
    Ok(())
}

#[cfg(not(windows))]
fn launch_app(path: &str) -> std::io::Result<()> {
    Command::new(path).spawn()?;
    Ok(())
}

fn main() {
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

    const MAX_RETRIES: u32 = 8;
    let mut retries = MAX_RETRIES;

    while retries > 0 {
        match fs::rename(new_path, old_path) {
            Ok(_) => {
                info!("✔ Successfully replaced binary.");
                break;
            }
            Err(e) => {
                retries -= 1;
                warn!(
                    "Failed to replace binary ({e}). Retries left: {}...",
                    retries
                );
                thread::sleep(Duration::from_millis(800));
            }
        }
    }

    if retries == 0 {
        error!("✖ Failed to replace binary after max retries.");
        std::process::exit(1);
    }

    info!("Restarting application: {}", old_path);
    if let Err(e) = launch_app(old_path) {
        error!("✖ Restart failed: {:?}", e);
        std::process::exit(1);
    }

    info!("Update complete.");
}
