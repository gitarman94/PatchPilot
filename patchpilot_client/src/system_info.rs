use anyhow::{Result, anyhow};
use serde_json::json;
use std::process::Command;

#[cfg(windows)]
mod windows {
    use super::*;

    pub fn get_serial_number() -> Result<String> {
        log::info!("Retrieving serial number for Windows device...");
        let output = Command::new("wmic")
            .args(["bios", "get", "serialnumber"])
            .output()
            .map_err(|e| anyhow!("Failed to execute WMIC: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Failed to retrieve serial number"));
        }

        let serial = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.contains("SerialNumber"))
            .next()
            .unwrap_or("")
            .trim()
            .to_string();

        if serial.is_empty() {
            Err(anyhow!("Serial number not found"))
        } else {
            Ok(serial)
        }
    }

    pub fn get_os_info() -> Result<String> {
        log::info!("Retrieving OS info for Windows device...");
        let output = Command::new("systeminfo")
            .args(["/fo", "CSV"])
            .output()
            .map_err(|e| anyhow!("Failed to execute systeminfo: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Failed to retrieve OS info"));
        }

        let info = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(info)
    }

    pub fn get_cpu_info() -> Result<f32> {
        log::info!("Retrieving CPU load...");
        let output = Command::new("wmic")
            .args(["cpu", "get", "loadpercentage"])
            .output()
            .map_err(|e| anyhow!("Failed to execute WMIC CPU: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Failed to retrieve CPU load"));
        }

        let cpu_str = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.contains("LoadPercentage"))
            .next()
            .unwrap_or("0")
            .trim()
            .to_string();

        Ok(cpu_str.parse::<f32>().unwrap_or(0.0))
    }

    pub fn get_memory_info() -> Result<serde_json::Value> {
        log::info!("Retrieving memory info...");
        let output = Command::new("systeminfo")
            .args(["/fo", "CSV"])
            .output()
            .map_err(|e| anyhow!("Failed to execute systeminfo for memory: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Failed to retrieve memory info"));
        }

        let csv = String::from_utf8_lossy(&output.stdout);
        let total_memory = csv.lines()
            .find(|line| line.contains("Total Physical Memory"))
            .and_then(|line| line.split(':').nth(1))
            .unwrap_or("0")
            .trim()
            .replace(",", "")
            .parse::<u64>()
            .unwrap_or(0);

        let free_memory = csv.lines()
            .find(|line| line.contains("Available Physical Memory"))
            .and_then(|line| line.split(':').nth(1))
            .unwrap_or("0")
            .trim()
            .replace(",", "")
            .parse::<u64>()
            .unwrap_or(0);

        Ok(json!({ "total_memory": total_memory, "free_memory": free_memory }))
    }

    pub fn get_device_type() -> String {
        let output = Command::new("wmic")
            .args(["computersystem", "get", "PCSystemType"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let val = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .filter(|line| !line.trim().is_empty() && !line.contains("PCSystemType"))
                    .next()
                    .unwrap_or("1")
                    .trim();

                match val {
                    "2" => "Laptop".to_string(),
                    "1" => "Desktop".to_string(),
                    _ => "Unknown".to_string(),
                }
            }
            _ => "Unknown".to_string(),
        }
    }

    pub fn get_device_model() -> String {
        let output = Command::new("wmic")
            .args(["computersystem", "get", "model"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .filter(|line| !line.trim().is_empty() && !line.contains("Model"))
                    .next()
                    .unwrap_or("Unknown Model")
                    .trim()
                    .to_string()
            }
            _ => "Unknown Model".to_string(),
        }
    }
}

#[cfg(unix)]
mod unix {
    use super::*;

    pub fn get_serial_number() -> Result<String> {
        log::info!("Retrieving serial number for Unix device...");
        let output = Command::new("dmidecode")
            .args(["-s", "system-serial-number"])
            .output()
            .map_err(|e| anyhow!("Failed to execute dmidecode: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Failed to retrieve serial number"));
        }

        let serial = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .next()
            .unwrap_or("")
            .trim()
            .to_string();

        if serial.is_empty() {
            Err(anyhow!("Serial number not found"))
        } else {
            Ok(serial)
        }
    }

    pub fn get_os_info() -> Result<String> {
        log::info!("Retrieving OS info...");
        let output = Command::new("uname")
            .arg("-a")
            .output()
            .map_err(|e| anyhow!("Failed to execute uname: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Failed to retrieve OS info"));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn get_cpu_info() -> Result<f32> {
        log::info!("Retrieving CPU load...");
        let output = Command::new("sh")
            .arg("-c")
            .arg("top -bn1 | grep 'Cpu(s)' | awk '{print 100 - $8}'")
            .output()
            .map_err(|e| anyhow!("Failed to execute top for CPU: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Failed to retrieve CPU load"));
        }

        Ok(String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<f32>()
            .unwrap_or(0.0))
    }

    pub fn get_memory_info() -> Result<serde_json::Value> {
        log::info!("Retrieving memory info...");
        let output = Command::new("free")
            .arg("-b")
            .output()
            .map_err(|e| anyhow!("Failed to execute free command: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Failed to retrieve memory info"));
        }

        let parts: Vec<&str> = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .collect();

        let total = parts.get(1).unwrap_or(&"0").parse::<u64>().unwrap_or(0);
        let free = parts.get(3).unwrap_or(&"0").parse::<u64>().unwrap_or(0);

        Ok(json!({ "total_memory": total, "free_memory": free }))
    }

    pub fn get_device_type() -> String {
        if std::path::Path::new("/sys/class/power_supply/BAT0").exists() {
            "Laptop".to_string()
        } else {
            "Desktop".to_string()
        }
    }

    pub fn get_device_model() -> String {
        let output = Command::new("cat")
            .arg("/sys/class/dmi/id/product_name")
            .output();

        match output {
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
                .trim()
                .to_string(),
            _ => "Unknown Model".to_string(),
        }
    }
}

pub fn get_system_info() -> Result<serde_json::Value> {
    #[cfg(windows)]
    {
        let serial_number = windows::get_serial_number()?;
        let os_info = windows::get_os_info()?;
        let cpu = windows::get_cpu_info()?;
        let memory = windows::get_memory_info()?;
        let device_type = windows::get_device_type();
        let device_model = windows::get_device_model();

        Ok(json!({
            "serial_number": serial_number,
            "os_info": os_info,
            "cpu": cpu,
            "memory": memory,
            "device_type": device_type,
            "device_model": device_model,
        }))
    }

    #[cfg(unix)]
    {
        let serial_number = unix::get_serial_number()?;
        let os_info = unix::get_os_info()?;
        let cpu = unix::get_cpu_info()?;
        let memory = unix::get_memory_info()?;
        let device_type = unix::get_device_type();
        let device_model = unix::get_device_model();

        Ok(json!({
            "serial_number": serial_number,
            "os_info": os_info,
            "cpu": cpu,
            "memory": memory,
            "device_type": device_type,
            "device_model": device_model,
        }))
    }
}
