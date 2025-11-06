use std::process::Command;
use anyhow::{Result, anyhow};
use serde_json::json;
use log::{info, error};

#[cfg(windows)]
mod windows {
    use super::*;

    pub fn get_system_info() -> Result<serde_json::Value> {
        info!("Retrieving system information for Windows...");

        let serial_number = get_serial_number()?;
        let os_info = get_os_info()?;
        let cpu_info = get_cpu_info()?;
        let memory_info = get_memory_info()?;

        Ok(json!( {
            "serial_number": serial_number,
            "os_info": os_info,
            "cpu": cpu_info,
            "memory": memory_info,
        }))
    }

    fn get_serial_number() -> Result<String> {
        info!("Getting serial number...");
        let output = Command::new("wmic")
            .args(&["bios", "get", "serialnumber"])
            .output()
            .map_err(|e| anyhow!("Failed to execute WMIC: {}", e))?;

        if !output.status.success() {
            error!("Failed to retrieve serial number using WMIC");
            return Err(anyhow!("Failed to retrieve serial number"));
        }

        let serial_number = String::from_utf8_lossy(&output.stdout)
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
        let output = Command::new("systeminfo")
            .arg("/fo")
            .arg("CSV")
            .output()
            .map_err(|e| anyhow!("Failed to execute systeminfo: {}", e))?;

        if !output.status.success() {
            error!("Failed to retrieve OS info using systeminfo");
            return Err(anyhow!("Failed to retrieve OS version"));
        }

        let os_info = String::from_utf8_lossy(&output.stdout).to_string();
        info!("OS info retrieved: {}", os_info);
        Ok(os_info)
    }

    fn get_cpu_info() -> Result<f32> {
        info!("Getting CPU load...");
        let output = Command::new("wmic")
            .args(&["cpu", "get", "loadpercentage"])
            .output()
            .map_err(|e| anyhow!("Failed to execute WMIC for CPU info: {}", e))?;

        if !output.status.success() {
            error!("Failed to retrieve CPU load using WMIC");
            return Err(anyhow!("Failed to retrieve CPU load"));
        }

        let cpu_load_str = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .last()
            .unwrap_or("0")
            .trim();

        let cpu_load: f32 = cpu_load_str.parse().unwrap_or(0.0);
        info!("CPU load retrieved: {}%", cpu_load);
        Ok(cpu_load)
    }

    fn get_memory_info() -> Result<serde_json::Value> {
        info!("Getting memory info...");
        let output = Command::new("systeminfo")
            .arg("/fo")
            .arg("CSV")
            .output()
            .map_err(|e| anyhow!("Failed to execute systeminfo for memory: {}", e))?;

        if !output.status.success() {
            error!("Failed to retrieve memory info using systeminfo");
            return Err(anyhow!("Failed to retrieve memory info"));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let total_memory = output_str.lines()
            .filter(|line| line.contains("Total Physical Memory"))
            .map(|line| line.split(":").nth(1).unwrap_or("").trim())
            .next()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);

        let free_memory = output_str.lines()
            .filter(|line| line.contains("Available Physical Memory"))
            .map(|line| line.split(":").nth(1).unwrap_or("").trim())
            .next()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);

        info!("Memory info retrieved: total_memory: {}, free_memory: {}", total_memory, free_memory);

        Ok(json!( {
            "total_memory": total_memory,
            "free_memory": free_memory
        }))
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::fs;

    pub fn get_system_info() -> Result<serde_json::Value> {
        info!("Retrieving system information for Unix...");

        let serial_number = get_serial_number()?;
        let os_info = get_os_info()?;
        let cpu_info = get_cpu_info()?;
        let memory_info = get_memory_info()?;

        Ok(json!( {
            "serial_number": serial_number,
            "os_info": os_info,
            "cpu": cpu_info,
            "memory": memory_info,
        }))
    }

    fn get_serial_number() -> Result<String> {
        info!("Getting serial number...");
        let output = Command::new("dmidecode")
            .args(&["-s", "system-serial-number"])
            .output()
            .map_err(|e| anyhow!("Failed to execute dmidecode: {}", e))?;

        if !output.status.success() {
            error!("Failed to retrieve serial number using dmidecode");
            return Err(anyhow!("Failed to retrieve serial number"));
        }

        let serial_number = String::from_utf8_lossy(&output.stdout)
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
        let output = Command::new("uname")
            .arg("-a")
            .output()
            .map_err(|e| anyhow!("Failed to execute uname: {}", e))?;

        if !output.status.success() {
            error!("Failed to retrieve OS info using uname");
            return Err(anyhow!("Failed to retrieve system information"));
        }

        let os_info = String::from_utf8_lossy(&output.stdout).to_string();
        info!("OS info retrieved: {}", os_info);
        Ok(os_info)
    }

    fn get_cpu_info() -> Result<f32> {
        info!("Getting CPU load...");
        let stat_content = fs::read_to_string("/proc/stat")
            .map_err(|e| anyhow!("Failed to read /proc/stat: {}", e))?;
        let mut total = 0u64;
        let mut idle = 0u64;

        if let Some(line) = stat_content.lines().next() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 5 {
                return Err(anyhow!("Unexpected /proc/stat format"));
            }
            let nums: Vec<u64> = parts[1..].iter().filter_map(|v| v.parse().ok()).collect();
            total = nums.iter().sum();
            idle = nums[3]; // idle time
        }

        // Wait 100ms and read again
        std::thread::sleep(std::time::Duration::from_millis(100));
        let stat_content2 = fs::read_to_string("/proc/stat")?;
        let mut total2 = 0u64;
        let mut idle2 = 0u64;
        if let Some(line) = stat_content2.lines().next() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let nums: Vec<u64> = parts[1..].iter().filter_map(|v| v.parse().ok()).collect();
            total2 = nums.iter().sum();
            idle2 = nums[3];
        }

        let total_delta = total2.saturating_sub(total);
        let idle_delta = idle2.saturating_sub(idle);
        let usage = if total_delta == 0 { 0.0 } else { ((total_delta - idle_delta) as f32 / total_delta as f32) * 100.0 };

        info!("CPU load retrieved: {:.2}%", usage);
        Ok(usage)
    }

    fn get_memory_info() -> Result<serde_json::Value> {
        info!("Getting memory info...");
        let output = Command::new("free")
            .arg("-b")
            .output()
            .map_err(|e| anyhow!("Failed to execute free command: {}", e))?;

        if !output.status.success() {
            error!("Failed to retrieve memory info using free");
            return Err(anyhow!("Failed to retrieve memory info"));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut total = 0u64;
        let mut free = 0u64;
        for line in output_str.lines() {
            if line.starts_with("Mem:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                total = parts.get(1).unwrap_or(&"0").parse().unwrap_or(0);
                free = parts.get(3).unwrap_or(&"0").parse().unwrap_or(0);
            }
        }

        info!("Memory info retrieved: total_memory: {}, free_memory: {}", total, free);
        Ok(json!( {
            "total_memory": total,
            "free_memory": free
        }))
    }
}
