use anyhow::{Result, anyhow};

#[cfg(windows)]
mod windows {
    use anyhow::{Result, anyhow};
    use std::process::Command;

    pub fn get_system_info() -> Result<String> {
        // Replace with your existing Windows system info logic or sysinfo crate
        Ok("Windows system info placeholder".to_string())
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
    use anyhow::Result;
    use sysinfo::{System, SystemExt};

    pub fn get_system_info() -> Result<String> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let info = format!(
            "OS: {:?}\nKernel Version: {:?}\nTotal Memory: {} KB\nCPU Count: {}",
            sys.name(),
            sys.kernel_version(),
            sys.total_memory(),
            sys.cpus().len()
        );

        Ok(info)
    }

    pub fn get_missing_windows_updates() -> Result<Vec<String>> {
        // Not applicable on Unix, return empty vec
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
