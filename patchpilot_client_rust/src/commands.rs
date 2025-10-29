use std::process::Command;
use anyhow::{Result, anyhow};
use serde_json::json;
use log::{info, error}; // Add logging imports

#[cfg(windows)]
mod windows {
    use super::*;

    pub fn get_system_info() -> Result<serde_json::Value> {
        info!("Retrieving system information for Windows...");
        
        let serial_number = get_serial_number()?;
        let os_info = get_os_info()?;
        let cpu_info = get_cpu_info()?;
        let memory_info = get_memory_info()?;

        let system_info = json!({
            "serial_number": serial_number,
            "os_info": os_info,
            "cpu": cpu_info,
            "memory": memory_info,
        });

        Ok(system_info)
    }

    fn get_serial_number() -> Result<String> {
        info!("Getting serial number...");
        let serial_number = Command::new("wmic")
            .arg("bios")
            .arg("get")
            .arg("serialnumber")
            .output()
            .map_err(|e| anyhow!("Failed to execute WMIC: {}", e))?;

        if !serial_number.status.success() {
            error!("Failed to retrieve serial number using WMIC");
            return Err(anyhow!("Failed to retrieve serial number"));
        }

        let serial_number = String::from_utf8_lossy(&serial_number.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .last()
            .unwrap_or("")
            .trim()
            .to_string();

        info!("Serial number retrieved: {}", serial_number);
        Ok(serial_number)
    }

    fn get_os_info() -> Result<String> {
        info!("Getting OS info...");
        let os_version = Command::new("systeminfo")
            .arg("/fo")
            .arg("CSV")
            .output()
            .map_err(|e| anyhow!("Failed to execute systeminfo: {}", e))?;

        if !os_version.status.success() {
            error!("Failed to retrieve OS info using systeminfo");
            return Err(anyhow!("Failed to retrieve OS version"));
        }

        let os_version_str = String::from_utf8_lossy(&os_version.stdout).to_string();
        info!("OS info retrieved: {}", os_version_str);
        Ok(os_version_str)
    }

    fn get_cpu_info() -> Result<f32> {
        info!("Getting CPU load...");
        let cpu_load = Command::new("wmic")
            .arg("cpu")
            .arg("get")
            .arg("loadpercentage")
            .output()
            .map_err(|e| anyhow!("Failed to execute WMIC for CPU info: {}", e))?;

        if !cpu_load.status.success() {
            error!("Failed to retrieve CPU load using WMIC");
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
        info!("CPU load retrieved: {}%", cpu_load);
        Ok(cpu_load)
    }

    fn get_memory_info() -> Result<serde_json::Value> {
        info!("Getting memory info...");
        let memory_stats = Command::new("systeminfo")
            .arg("/fo")
            .arg("CSV")
            .output()
            .map_err(|e| anyhow!("Failed to execute systeminfo for memory: {}", e))?;

        if !memory_stats.status.success() {
            error!("Failed to retrieve memory info using systeminfo");
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

        info!("Memory info retrieved: total_memory: {}, free_memory: {}", total_memory, free_memory);
        Ok(memory_info)
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::process::Command;

    pub fn get_system_info() -> Result<serde_json::Value> {
        info!("Retrieving system information for Unix...");
        
        let serial_number = get_serial_number()?;
        let os_info = get_os_info()?;
        let cpu_info = get_cpu_info()?;
        let memory_info = get_memory_info()?;

        let system_info = json!({
            "serial_number": serial_number,
            "os_info": os_info,
            "cpu": cpu_info,
            "memory": memory_info,
        });

        Ok(system_info)
    }

    fn get_serial_number() -> Result<String> {
        info!("Getting serial number...");
        let serial_number = Command::new("dmidecode")
            .arg("-s")
            .arg("system-serial-number")
            .output()
            .map_err(|e| anyhow!("Failed to execute dmidecode: {}", e))?;

        if !serial_number.status.success() {
            error!("Failed to retrieve serial number using dmidecode");
            return Err(anyhow!("Failed to retrieve serial number"));
        }

        let serial_number = String::from_utf8_lossy(&serial_number.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .last()
            .unwrap_or("")
            .trim()
            .to_string();

        info!("Serial number retrieved: {}", serial_number);
        Ok(serial_number)
    }

    fn get_os_info() -> Result<String> {
        info!("Getting OS info...");
        let uname_output = Command::new("uname")
            .arg("-a")
            .output()
            .map_err(|e| anyhow!("Failed to execute uname: {}", e))?;

        if !uname_output.status.success() {
            error!("Failed to retrieve OS info using uname");
            return Err(anyhow!("Failed to retrieve system information"));
        }

        let system_info = String::from_utf8_lossy(&uname_output.stdout).to_string();
        info!("OS info retrieved: {}", system_info);
        Ok(system_info)
    }

    fn get_cpu_info() -> Result<f32> {
        info!("Getting CPU load...");
        let cpu_load = Command::new("top")
            .arg("-b")
            .arg("-n1")
            .arg("|")
            .arg("grep")
            .arg("Cpu(s)")
            .arg("|")
            .arg("sed")
            .arg("'s/.*, *\([0-9.]*\)%* id.*/\\1/'")
            .arg("|")
            .arg("awk")
            .arg("'BEGIN {print 100 - $1}'")
            .output()
            .map_err(|e| anyhow!("Failed to execute top command for CPU load: {}", e))?;

        if !cpu_load.status.success() {
            error!("Failed to retrieve CPU load using top");
            return Err(anyhow!("Failed to retrieve CPU load"));
        }

        let cpu_load_str = String::from_utf8_lossy(&cpu_load.stdout)
            .trim()
            .to_string();

        let cpu_load: f32 = cpu_load_str.parse().unwrap_or(0.0);
        info!("CPU load retrieved: {}%", cpu_load);
        Ok(cpu_load)
    }

    fn get_memory_info() -> Result<serde_json::Value> {
        info!("Getting memory info...");
        let free_memory = Command::new("free")
            .arg("-b")
            .output()
            .map_err(|e| anyhow!("Failed to execute free command: {}", e))?;

        if !free_memory.status.success() {
            error!("Failed to retrieve memory info using free");
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

        info!("Memory info retrieved: total_memory: {}, free_memory: {}", total_memory, free_memory);
        Ok(memory_info)
    }
}

#[cfg(not(windows))]
pub fn get_system_info() -> Result<serde_json::Value> {
    unix::get_system_info()
}

#[cfg(windows)]
pub fn get_system_info() -> Result<serde_json::Value> {
    windows::get_system_info()
}
