#[cfg(windows)]
fn check_antivirus() -> Result<()> {
    let ps_script = r#"
        $antivirus = Get-WmiObject -Namespace "root\CIMv2" -Class AntiVirusProduct
        if ($antivirus) { exit 0 } else { exit 1 }
    "#;

    let status = Command::new("powershell")
        .args(&["-Command", ps_script])
        .status()
        .map_err(|e| anyhow!("Failed to check antivirus: {}", e))?;

    if !status.success() {
        return Err(anyhow!("Antivirus check failed"));
    }

    Ok(())
}

#[cfg(unix)]
fn check_antivirus() -> Result<()> {
    let status = Command::new("sh")
        .arg("-c")
        .arg("pgrep -x 'antivirus-daemon' > /dev/null")
        .status()
        .map_err(|e| anyhow!("Failed to check antivirus: {}", e))?;

    if !status.success() {
        return Err(anyhow!("Antivirus check failed"));
    }

    Ok(())
}

#[cfg(windows)]
fn get_installed_software() -> Result<Vec<String>> {
    let ps_script = r#"
        Get-WmiObject -Class Win32_Product | Select-Object Name, Version
    "#;

    let output = Command::new("powershell")
        .args(&["-Command", ps_script])
        .output()
        .map_err(|e| anyhow!("Failed to get installed software: {}", e))?;

    if !output.status.success() {
        return Err(anyhow!("Failed to get installed software"));
    }

    let software_list = parse_software_output(&output.stdout);
    Ok(software_list)
}

fn parse_software_output(output: &[u8]) -> Vec<String> {
    // Parse the PowerShell output and return a list of installed software
    vec!["Software1".to_string(), "Software2".to_string()] // Example
}
