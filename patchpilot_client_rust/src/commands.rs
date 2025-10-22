use anyhow::{Result, anyhow};
use std::process::Command;

pub fn execute_command(command: &str, args: &[String]) -> Result<()> {
    match command {
        "install_updates" => {
            install_updates(args)?;
        }
        "install_all_updates" => {
            install_updates(&[])?; // Install all available updates
        }
        "reboot" => {
            reboot_system()?;
        }
        "shutdown" => {
            shutdown_system()?;
        }
        _ => {
            println!("Unknown command: {}", command);
        }
    }
    Ok(())
}

#[cfg(windows)]
fn install_updates(update_titles: &[String]) -> Result<()> {
    let ps_script = if update_titles.is_empty() {
        r#"
        $Session = New-Object -ComObject Microsoft.Update.Session
        $Installer = $Session.CreateUpdateInstaller()
        $Searcher = $Session.CreateUpdateSearcher()
        $SearchResult = $Searcher.Search("IsInstalled=0 and Type='Software'")
        $Installer.Updates = $SearchResult.Updates
        $InstallationResult = $Installer.Install()
        if ($InstallationResult.ResultCode -eq 2) { exit 0 } else { exit 1 }
        "#.to_string()
    } else {
        let updates_array = update_titles
            .iter()
            .map(|title| format!("'{}'", title.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            r#"
            $titles = @({updates})
            $Session = New-Object -ComObject Microsoft.Update.Session
            $Installer = $Session.CreateUpdateInstaller()
            $Searcher = $Session.CreateUpdateSearcher()
            $SearchResult = $Searcher.Search("IsInstalled=0 and Type='Software'")
            $UpdatesToInstall = New-Object -ComObject Microsoft.Update.UpdateColl

            foreach ($update in $SearchResult.Updates) {{
                if ($titles -contains $update.Title) {{
                    $UpdatesToInstall.Add($update) | Out-Null
                }}
            }}

            $Installer.Updates = $UpdatesToInstall
            if ($UpdatesToInstall.Count -eq 0) {{ exit 0 }}

            $InstallationResult = $Installer.Install()
            if ($InstallationResult.ResultCode -eq 2) {{ exit 0 }} else {{ exit 1 }}
            "#,
            updates = updates_array
        )
    };

    let status = Command::new("powershell")
        .args(&["-NoProfile", "-Command", &ps_script])
        .status()
        .map_err(|e| anyhow!("Failed to run PowerShell install script: {}", e))?;

    if !status.success() {
        return Err(anyhow!("Update installation failed"));
    }

    Ok(())
}

#[cfg(unix)]
fn install_updates(_update_titles: &[String]) -> Result<()> {
    // Linux update installation logic here, e.g.:
    // Call "apt-get update && apt-get upgrade -y" or appropriate package manager
    let status = Command::new("sh")
        .arg("-c")
        .arg("sudo apt-get update && sudo apt-get upgrade -y")
        .status()?;

    if !status.success() {
        return Err(anyhow!("Failed to install updates"));
    }

    Ok(())
}

#[cfg(windows)]
fn reboot_system() -> Result<()> {
    Command::new("shutdown")
        .args(&["/r", "/t", "0"])
        .status()?;
    Ok(())
}

#[cfg(unix)]
fn reboot_system() -> Result<()> {
    Command::new("sudo")
        .arg("reboot")
        .status()?;
    Ok(())
}

#[cfg(windows)]
fn shutdown_system() -> Result<()> {
    Command::new("shutdown")
        .args(&["/s", "/t", "0"])
        .status()?;
    Ok(())
}

#[cfg(unix)]
fn shutdown_system() -> Result<()> {
    Command::new("sudo")
        .arg("shutdown")
        .arg("-h")
        .arg("now")
        .status()?;
    Ok(())
}
