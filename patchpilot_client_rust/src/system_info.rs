use anyhow::{Result, anyhow};
use std::process::Command;

#[cfg(windows)]
mod windows {
    use super::*;

    pub fn get_system_info() -> Result<String> {
        // Get the serial number on Windows using WMIC command
        let serial_number = Command::new("wmic")
            .arg("bios")
            .arg("get")
            .arg("serialnumber")
            .output()
            .map_err(|e| anyhow!("Failed to execute WMIC: {}", e))?;

        if !serial_number.status.success() {
            return Err(anyhow!("Failed to retrieve serial number"));
        }

        let serial_number = String::from_utf8_lossy(&serial_number.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .last()
            .unwrap_or("")
            .trim()
            .to_string();

        // Getting other system information like OS, CPU, RAM, etc.
        let os_version = Command::new("systeminfo")
            .arg("/fo")
            .arg("CSV")
            .output()
            .map_err(|e| anyhow!("Failed to execute systeminfo: {}", e))?;
        
        if !os_version.status.success() {
            return Err(anyhow!("Failed to retrieve OS version"));
        }

        let os_version_str = String::from_utf8_lossy(&os_version.stdout);

        Ok(format!(
            "Windows Serial Number: {}\nOS Information: {}",
            serial_number, os_version_str
        ))
    }

    pub fn get_missing_windows_updates() -> Result<Vec<String>> {
        let ps_script = r#"
            $Session = New-Object -ComObject Microsoft.Update.Session
            $Searcher = $Session.CreateUpdateSearcher()
            $SearchResult = $Searcher.Search("IsInstalled=0 and Type='Software'")
            $updates = $SearchResult.Updates | ForEach-Object { $_.Title }
            $updates -join "`n"
        "#;

        let output = Command::new("powershell")
            .args(&["-NoProfile", "-Command", ps_script])
            .output()
            .map_err(|e| anyhow!("Failed to execute PowerShell: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("PowerShell script failed: {}", String::from_utf8_lossy(&output.stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let updates: Vec<String> = stdout
            .lines()
            .map(str::to_string)
            .filter(|line| !line.trim().is_empty())
            .collect();

        Ok(updates)
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::process::Command;

    pub fn get_system_info() -> Result<String> {
        // Getting serial number on Unix-based systems
        let serial_number = Command::new("dmidecode")
            .arg("-s")
            .arg("system-serial-number")
            .output()
            .map_err(|e| anyhow!("Failed to execute dmidecode: {}", e))?;

        if !serial_number.status.success() {
            return Err(anyhow!("Failed to retrieve serial number"));
        }

        let serial_number = String::from_utf8_lossy(&serial_number.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .last()
            .unwrap_or("")
            .trim()
            .to_string();

        // Gather other system information (OS, CPU, RAM, etc.)
        let uname_output = Command::new("uname")
            .arg("-a")
            .output()
            .map_err(|e| anyhow!("Failed to execute uname: {}", e))?;
        
        if !uname_output.status.success() {
            return Err(anyhow!("Failed to retrieve system information"));
        }

        let system_info = String::from_utf8_lossy(&uname_output.stdout).to_string();

        Ok(format!(
            "Unix Serial Number: {}\nSystem Info: {}",
            serial_number, system_info
        ))
    }

    pub fn get_missing_windows_updates() -> Result<Vec<String>> {
        // Not applicable on Unix systems
        Ok(vec![])
    }
}

// Public interface

pub fn get_system_info() -> Result<String> {
    #[cfg(windows)]
    {
        windows::get_system_info()
    }
    #[cfg(unix)]
    {
        unix::get_system_info()
    }
}

pub fn get_missing_windows_updates() -> Result<Vec<String>> {
    #[cfg(windows)]
    {
        windows::get_missing_windows_updates()
    }
    #[cfg(unix)]
    {
        unix::get_missing_windows_updates()
    }
}
