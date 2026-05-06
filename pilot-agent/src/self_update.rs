use anyhow::{bail, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{
    env,
    fs,
    path::PathBuf,
    process::{Command, exit},
    time::Duration,
};

#[derive(Deserialize, Debug)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize, Debug)]
struct ReleaseInfo {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

const GITHUB_USER: &str = "gitarman94";
const GITHUB_REPO: &str = "PatchPilot";

// ---- Executable Names ----

#[cfg(target_os = "windows")]
const EXE_NAME: &str = "rust_patch_client.exe";

#[cfg(not(target_os = "windows"))]
const EXE_NAME: &str = "rust_patch_client";

// ---- Updater Names ----

#[cfg(target_os = "windows")]
const UPDATER_NAME: &str = "patchpilot_updater.exe";

#[cfg(not(target_os = "windows"))]
const UPDATER_NAME: &str = "patchpilot_updater";

// ---- Runtime Base Directory ----

#[cfg(target_os = "windows")]
const RUNTIME_DIR: &str = "C:\\ProgramData\\PatchPilot";

#[cfg(all(unix, not(target_os = "macos")))]
const RUNTIME_DIR: &str = "/opt/patchpilot_client";

#[cfg(target_os = "macos")]
const RUNTIME_DIR: &str = "/Library/Application Support/PatchPilot";


/// Checks GitHub releases and updates the agent if needed
pub fn check_and_update() -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;

    let current_version = env!("CARGO_PKG_VERSION");
    log::info!("Current version: {}", current_version);

    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        GITHUB_USER, GITHUB_REPO
    );

    let resp = client
        .get(&url)
        .header("User-Agent", "PatchPilotUpdater")
        .send()?
        .error_for_status()?
        .json::<ReleaseInfo>()?;

    let latest_version = resp.tag_name.as_str();

    if latest_version == current_version {
        log::info!("Already up-to-date: {}", latest_version);
        return Ok(());
    }

    log::info!("🚀 New version available: {}", latest_version);

    // Find the correct binary asset for this OS
    let asset = resp
        .assets
        .iter()
        .find(|a| a.name == EXE_NAME)
        .ok_or_else(|| anyhow::anyhow!("Executable '{}' not found in release assets", EXE_NAME))?;

    log::info!("Found asset: {}", asset.browser_download_url);

    // Download into the runtime directory, not /tmp
    let new_exe_path = PathBuf::from(RUNTIME_DIR).join(format!("{}.new", EXE_NAME));

    fs::create_dir_all(RUNTIME_DIR).ok();
    download_file(&client, &asset.browser_download_url, &new_exe_path)?;

    log::info!(
        "Downloaded new version to {}",
        new_exe_path.display()
    );

    // Determine updater path
    let updater_path = PathBuf::from(RUNTIME_DIR).join(UPDATER_NAME);

    if !updater_path.exists() {
        bail!(
            "Updater not found at {} — cannot continue",
            updater_path.display()
        );
    }

    log::info!("Launching updater: {}", updater_path.display());

    // Arguments: <old_exe> <new_exe>
    let old_exe_path = env::current_exe()?;

    let status = Command::new(&updater_path)
        .arg(old_exe_path)
        .arg(&new_exe_path)
        .status()?;

    if !status.success() {
        bail!("Updater failed to execute successfully");
    }

    log::info!("Update launched — shutting down current client…");
    exit(0);
}


/// Download a file to a local path
fn download_file(client: &Client, url: &str, dest: &PathBuf) -> Result<()> {
    log::info!("Downloading: {}", url);

    let mut resp = client
        .get(url)
        .header("User-Agent", "PatchPilotUpdater")
        .send()?
        .error_for_status()?;

    log::info!("Saving to: {}", dest.display());

    let mut file = fs::File::create(dest)?;
    std::io::copy(&mut resp, &mut file)?;

    log::info!("Download complete.");
    Ok(())
}
