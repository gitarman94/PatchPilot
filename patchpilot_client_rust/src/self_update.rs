use anyhow::{bail, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{
    env,
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, exit},
    thread,
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

// Adjust these constants per your repo and executable names
const GITHUB_USER: &str = "<your-github-username>";
const GITHUB_REPO: &str = "<your-rust-client-repo>";

#[cfg(windows)]
const EXE_NAME: &str = "rust_patch_client.exe";

#[cfg(not(windows))]
const EXE_NAME: &str = "rust_patch_client";

#[cfg(windows)]
const UPDATER_NAME: &str = "rust_patch_updater.exe";

#[cfg(not(windows))]
const UPDATER_NAME: &str = "rust_patch_updater";

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

    let resp = client.get(&url)
        .header("User-Agent", "RustPatchClientUpdater")
        .send()?
        .error_for_status()?
        .json::<ReleaseInfo>()?;

    let latest_version = resp.tag_name.as_str();
    if latest_version == current_version {
        log::info!("Already on latest version.");
        return Ok(());
    }

    log::info!("Found new version: {}", latest_version);

    let asset = resp.assets.iter()
        .find(|a| a.name == EXE_NAME)
        .ok_or_else(|| anyhow::anyhow!("Executable asset not found"))?;

    let tmp_dir = env::temp_dir();
    let new_exe_path = tmp_dir.join(EXE_NAME);
    download_file(&client, &asset.browser_download_url, &new_exe_path)?;

    // Launch updater helper
    let updater_path = env::current_exe()?
        .parent().unwrap().join(UPDATER_NAME);

    let status = Command::new(&updater_path)
        .arg(env::current_exe()?)
        .arg(new_exe_path)
        .status()?;

    if !status.success() {
        bail!("Updater helper failed");
    }

    log::info!("Update launched – exiting current version.");
    exit(0);
}

fn download_file(client: &Client, url: &str, dest: &PathBuf) -> Result<()> {
    log::info!("Downloading from {}", url);
    let mut resp = client.get(url)
        .header("User-Agent", "RustPatchClientUpdater")
        .send()?
        .error_for_status()?;
    let mut file = fs::File::create(dest)?;
    std::io::copy(&mut resp, &mut file)?;
    Ok(())
}
