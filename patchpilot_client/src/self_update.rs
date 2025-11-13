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

#[cfg(windows)]
const EXE_NAME: &str = "rust_patch_client.exe";
#[cfg(not(windows))]
const EXE_NAME: &str = "rust_patch_client";

#[cfg(windows)]
const UPDATER_NAME: &str = "patchpilot_updater.exe";
#[cfg(not(windows))]
const UPDATER_NAME: &str = "patchpilot_updater";

/// Checks for a newer release on GitHub and launches updater if needed
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
        log::info!("Already on the latest version: {}", latest_version);
        return Ok(());
    }

    log::info!("New version found: {}", latest_version);

    // Find the correct executable asset
    let asset = resp.assets.iter()
        .find(|a| a.name == EXE_NAME)
        .ok_or_else(|| anyhow::anyhow!("Executable asset not found in release"))?;

    log::info!("Downloading new executable: {}", asset.browser_download_url);

    let tmp_dir = env::temp_dir();
    let new_exe_path = tmp_dir.join(EXE_NAME);
    download_file(&client, &asset.browser_download_url, &new_exe_path)?;

    // Determine updater path
    let updater_path = env::current_exe()?
        .parent()
        .expect("Executable must have a parent directory")
        .join(UPDATER_NAME);

    log::info!("Launching updater: {}", updater_path.display());

    let status = Command::new(&updater_path)
        .arg(env::current_exe()?)
        .arg(&new_exe_path)
        .status()?;

    if !status.success() {
        bail!("Updater helper failed to launch");
    }

    log::info!("Update launched successfully — exiting current version.");
    exit(0);
}

/// Download a file to a local path
fn download_file(client: &Client, url: &str, dest: &PathBuf) -> Result<()> {
    log::info!("Downloading from: {}", url);

    let mut resp = client
        .get(url)
        .header("User-Agent", "PatchPilotUpdater")
        .send()?
        .error_for_status()?;

    log::info!("Saving file to: {}", dest.display());
    let mut file = fs::File::create(dest)?;
    std::io::copy(&mut resp, &mut file)?;
    log::info!("Download complete.");
    Ok(())
}
