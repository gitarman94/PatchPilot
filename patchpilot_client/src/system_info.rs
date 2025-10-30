use anyhow::{Result, anyhow};
use serde_json::json;
use std::process::Command;

#[cfg(windows)]
mod windows {
    use super::*;

    pub fn get_serial_number() -> Result<String> {
        log::info!("Retrieving serial number for Windows device...");
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

        log::info!("Retrieved serial number: {}", serial_number);
        Ok(serial_number)
    }

    pub fn get_os_info() -> Result<String> {
        log::info!("Retrieving OS information for Windows device...");
        let os_version = Command::new("systeminfo")
            .arg("/fo")
            .arg("CSV")
            .output()
            .map_err(|e| anyhow!("Failed to execute systeminfo: {}", e))?;

        if !os_version.status.success() {
            return Err(anyhow!("Failed to retrieve OS version"));
        }

        let os_version_str = String::from_utf8_lossy(&os_version.stdout).to_string();
        log::info!("Retrieved OS info:\n{}", os_version_str);
        Ok(os_version_str)
    }

    pub fn get_cpu_info() -> Result<f32> {
        log::info!("Retrieving CPU load for Windows device...");
        let cpu_load = Command::new("wmic")
            .arg("cpu")
            .arg("get")
            .arg("loadpercentage")
            .output()
            .map_err(|e| anyhow!("Failed to execute WMIC for CPU info: {}", e))?;

        if !cpu_load.status.success() {
            return Err(anyhow!("Failed to retrieve CPU load"));
        }

        let cpu_load_str = String::from_utf8_lossy(&cpu_load.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .last()
            .unwrap_or("0")
            .trim()
            .to_string();

        let cpu_load: f32 = cpu_load_str.parse().unwrap_or(0.0);
        log::info!("Retrieved CPU load: {}%", cpu_load);
        Ok(cpu_load)
    }

    pub fn get_memory_info() -> Result<serde_json::Value> {
        log::info!("Retrieving memory info for Windows device...");
        let memory_stats = Command::new("systeminfo")
            .arg("/fo")
            .arg("CSV")
            .output()
            .map_err(|e| anyhow!("Failed to execute systeminfo for memory: {}", e))?;

        if !memory_stats.status.success() {
            return Err(anyhow!("Failed to retrieve memory info"));
        }

        let output = String::from_utf8_lossy(&memory_stats.stdout);
        let total_memory = output.lines()
            .filter(|line| line.contains("Total Physical Memory"))
            .map(|line| line.split(":").nth(1).unwrap_or("").trim())
            .next()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);

        let free_memory = output.lines()
            .filter(|line| line.contains("Available Physical Memory"))
            .map(|line| line.split(":").nth(1).unwrap_or("").trim())
            .next()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);

        let memory_info = json!({
            "total_memory": total_memory,
            "free_memory": free_memory
        });

        log::info!("Retrieved memory info: {:?}", memory_info);
        Ok(memory_info)
    }
}

#[cfg(unix)]
mod unix {
    use super::*;

    pub fn get_serial_number() -> Result<String> {
        log::info!("Retrieving serial number for Unix device...");
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

        log::info!("Retrieved serial number: {}", serial_number);
        Ok(serial_number)
    }

    pub fn get_os_info() -> Result<String> {
        log::info!("Retrieving OS information for Unix device...");
        let uname_output = Command::new("uname")
            .arg("-a")
            .output()
            .map_err(|e| anyhow!("Failed to execute uname: {}", e))?;

        if !uname_output.status.success() {
            return Err(anyhow!("Failed to retrieve system information"));
        }

        let system_info = String::from_utf8_lossy(&uname_output.stdout).to_string();
        log::info!("Retrieved system info: {}", system_info);
        Ok(system_info)
    }

    pub fn get_cpu_info() -> Result<f32> {
        log::info!("Retrieving CPU load for Unix device...");
        let cpu_load = Command::new("top")
            .arg("-b")
            .arg("-n1")
            .arg("|")
            .arg("grep")
            .arg("Cpu(s)")
            .arg("|")
            .arg("sed")
            .arg(r"'s/.*, *\([0-9.]*\)%* id.*/\\1/'")
            .arg("|")
            .arg("awk")
            .arg("'BEGIN {print 100 - $1}'")
            .output()
            .map_err(|e| anyhow!("Failed to execute top command for CPU load: {}", e))?;

        if !cpu_load.status.success() {
            return Err(anyhow!("Failed to retrieve CPU load"));
        }

        let cpu_load_str = String::from_utf8_lossy(&cpu_load.stdout)
            .trim()
            .to_string();

        let cpu_load: f32 = cpu_load_str.parse().unwrap_or(0.0);
        log::info!("Retrieved CPU load: {}%", cpu_load);
        Ok(cpu_load)
    }

    pub fn get_memory_info() -> Result<serde_json::Value> {
        log::info!("Retrieving memory info for Unix device...");
        let free_memory = Command::new("free")
            .arg("-b")
            .output()
            .map_err(|e| anyhow!("Failed to execute free command: {}", e))?;

        if !free_memory.status.success() {
            return Err(anyhow!("Failed to retrieve memory info"));
        }

        let free_memory_str = String::from_utf8_lossy(&free_memory.stdout);
        let memory_data: Vec<&str> = free_memory_str.split_whitespace().collect();
        let total_memory = memory_data.get(1).unwrap_or(&"0").parse::<u64>().unwrap_or(0);
        let free_memory = memory_data.get(3).unwrap_or(&"0").parse::<u64>().unwrap_or(0);

        let memory_info = json!({
            "total_memory": total_memory,
            "free_memory": free_memory
        });

        log::info!("Retrieved memory info: {:?}", memory_info);
        Ok(memory_info)
    }
}

// Unified function to get system info, uses the platform-specific modules
pub fn get_system_info() -> Result<serde_json::Value> {
    #[cfg(windows)]
    {
        log::info!("Getting system info for Windows device...");
        let serial_number = windows::get_serial_number()?;
        let os_info = windows::get_os_info()?;
        let cpu_info = windows::get_cpu_info()?;
        let memory_info = windows::get_memory_info()?;

        let system_info = json!({
            "serial_number": serial_number,
            "os_info": os_info,
            "cpu": cpu_info,
            "memory": memory_info,
        });

        log::info!("Retrieved system info: {:?}", system_info);
        Ok(system_info)
    }

    #[cfg(unix)]
    {
        log::info!("Getting system info for Unix device...");
        let serial_number = unix::get_serial_number()?;
        let os_info = unix::get_os_info()?;
        let cpu_info = unix::get_cpu_info()?;
        let memory_info = unix::get_memory_info()?;

        let system_info = json!({
            "serial_number": serial_number,
            "os_info": os_info,
            "cpu": cpu_info,
            "memory": memory_info,
        });

        log::info!("Retrieved system info: {:?}", system_info);
        Ok(system_info)
    }
}
