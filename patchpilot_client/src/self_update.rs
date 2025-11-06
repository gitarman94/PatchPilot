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

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct ReleaseInfo {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

// Replace with your actual GitHub username and repo name
const GITHUB_USER: &str = "gitarman94";
const GITHUB_REPO: &str = "PatchPilot";

#[cfg(windows)]
const EXE_NAME: &str = "rust_patch_device.exe"; // Renamed executable to "device"

#[cfg(not(windows))]
const EXE_NAME: &str = "rust_patch_device"; // Renamed executable to "device"

#[cfg(windows)]
const UPDATER_NAME: &str = "rust_patch_updater.exe";

#[cfg(not(windows))]
const UPDATER_NAME: &str = "rust_patch_updater";

/// Checks if an update is available and performs the update if necessary
pub fn check_and_update() -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;

    let current_version = env!("CARGO_PKG_VERSION");
    log::info!("Current version: {}", current_version);

    // Check GitHub for the latest release
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        GITHUB_USER, GITHUB_REPO
    );

    let resp = client.get(&url)
        .header("User-Agent", "RustPatchDeviceUpdater")
        .send()?
        .error_for_status()?
        .json::<ReleaseInfo>()?;

    let latest_version = resp.tag_name.as_str();
    if latest_version == current_version {
        log::info!("Already on the latest version: {}", latest_version);
        return Ok(());
    }

    log::info!("Found new version: {}", latest_version);

    // Find the release asset for the correct executable
    let asset = resp.assets.iter()
        .find(|a| a.name == EXE_NAME)
        .ok_or_else(|| anyhow::anyhow!("Executable asset not found"))?;

    log::info!("Downloading new executable: {}", asset.browser_download_url);

    // Prepare the new executable file path
    let tmp_dir = env::temp_dir();
    let new_exe_path = tmp_dir.join(EXE_NAME);
    download_file(&client, &asset.browser_download_url, &new_exe_path)?;

    // Path to the updater executable
    let updater_path = env::current_exe()?
        .parent().expect("Executable must have a parent directory")
        .join(UPDATER_NAME);

    log::info!("Launching updater: {}", updater_path.display());

    // Launch the updater to perform the update
    let status = Command::new(&updater_path)
        .arg(env::current_exe()?)
        .arg(&new_exe_path)
        .status()?;

    if !status.success() {
        bail!("Updater helper failed");
    }

    log::info!("Update launched – exiting current version.");
    exit(0);
}

/// Downloads a file from the given URL and saves it to the specified path
fn download_file(client: &Client, url: &str, dest: &PathBuf) -> Result<()> {
    log::info!("Downloading file from: {}", url);
    
    let mut resp = client.get(url)
        .header("User-Agent", "RustPatchDeviceUpdater")
        .send()?
        .error_for_status()?;

    log::info!("Saving file to: {}", dest.display());
    
    let mut file = fs::File::create(dest)?;
    std::io::copy(&mut resp, &mut file)?;

    log::info!("Download complete.");
    Ok(())
}
