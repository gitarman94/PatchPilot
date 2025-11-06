use std::process::Command;
use anyhow::{Result, anyhow};
use serde_json::json;
use log::{info, error};
use crate::models::{DeviceInfo, SystemInfo}; // Add imports for DeviceInfo and SystemInfo

#[cfg(windows)]
mod windows {
    use super::*;
    use crate::system_info;

    pub fn get_system_info() -> Result<DeviceInfo> {
        info!("Retrieving system information for Windows...");

        // Call to system_info.rs for detailed system data
        let system_info = system_info::get_system_info()?; // Centralized logic call

        Ok(DeviceInfo {
            system_info,
            device_type: get_device_type()?,
            device_model: get_device_model()?,
        })
    }

    fn get_device_type() -> Result<String> {
        info!("Getting device type for Windows...");
        // Windows-specific logic for device type
        Ok("Desktop".to_string()) // Placeholder value
    }

    fn get_device_model() -> Result<String> {
        info!("Getting device model for Windows...");
        // Windows-specific logic for device model
        Ok("Surface Pro 7".to_string()) // Placeholder value
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use crate::system_info;

    pub fn get_system_info() -> Result<DeviceInfo> {
        info!("Retrieving system information for Unix...");

        // Call to system_info.rs for detailed system data
        let system_info = system_info::get_system_info()?; // Centralized logic call

        Ok(DeviceInfo {
            system_info,
            device_type: get_device_type()?,
            device_model: get_device_model()?,
        })
    }

    fn get_device_type() -> Result<String> {
        info!("Getting device type for Unix...");
        // Unix-specific logic for device type
        Ok("Laptop".to_string()) // Placeholder value
    }

    fn get_device_model() -> Result<String> {
        info!("Getting device model for Unix...");
        // Unix-specific logic for device model
        Ok("Dell XPS 13".to_string()) // Placeholder value
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use crate::system_info;

    pub fn get_system_info() -> Result<DeviceInfo> {
        info!("Retrieving system information for macOS...");

        // Call to system_info.rs for detailed system data
        let system_info = system_info::get_system_info()?; // Centralized logic call

        Ok(DeviceInfo {
            system_info,
            device_type: get_device_type()?,
            device_model: get_device_model()?,
        })
    }

    fn get_device_type() -> Result<String> {
        info!("Getting device type for macOS...");
        // macOS-specific logic for device type
        Ok("Laptop".to_string()) // Placeholder value
    }

    fn get_device_model() -> Result<String> {
        info!("Getting device model for macOS...");
        // macOS-specific logic for device model
        Ok("MacBook Pro".to_string()) // Placeholder value
    }
}

#[cfg(windows)]
mod windows_info {
    use super::*;
    use std::process::Command;

    fn get_cpu_info() -> Result<f32> {
        info!("Getting CPU info for Windows...");
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

    fn get_memory_info() -> Result<SystemInfo> {
        info!("Getting memory info for Windows...");
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

        Ok(SystemInfo {
            os_name: "Windows".to_string(),
            architecture: "x86_64".to_string(),
            cpu: 0.0, // Placeholder
            ram_total: total_memory as i64,
            ram_used: (total_memory - free_memory) as i64,
            ram_free: free_memory as i64,
            disk_total: 0,
            disk_free: 0,
            disk_health: "Healthy".to_string(),
            network_throughput: 0,
            ping_latency: None,
            network_interfaces: None,
            ip_address: None,
        })
    }

    fn get_serial_number() -> Result<String> {
        info!("Getting serial number for Windows...");
        let output = Command::new("wmic")
            .args(&["bios", "get", "serialnumber"])
            .output()
            .map_err(|e| anyhow!("Failed to execute WMIC for serial number: {}", e))?;

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
}

#[cfg(unix)]
mod unix_info {
    use super::*;
    use std::process::Command;

    fn get_cpu_info() -> Result<f32> {
        info!("Getting CPU info for Unix...");
        let output = Command::new("top")
            .args(&["-bn1"])
            .output()
            .map_err(|e| anyhow!("Failed to execute top for CPU info: {}", e))?;

        if !output.status.success() {
            error!("Failed to retrieve CPU info using top");
            return Err(anyhow!("Failed to retrieve CPU load"));
        }

        let cpu_info = String::from_utf8_lossy(&output.stdout)
            .lines()
            .find(|line| line.contains("Cpu(s)"))
            .map(|line| {
                line.split('%')
                    .next()
                    .unwrap_or("0")
                    .trim()
                    .parse::<f32>()
                    .unwrap_or(0.0)
            })
            .unwrap_or(0.0);

        info!("CPU load retrieved: {}%", cpu_info);
        Ok(cpu_info)
    }

    fn get_memory_info() -> Result<SystemInfo> {
        info!("Getting memory info for Unix...");
        let output = Command::new("free")
            .arg("-b")
            .output()
            .map_err(|e| anyhow!("Failed to execute free for memory: {}", e))?;

        if !output.status.success() {
            error!("Failed to retrieve memory info using free");
            return Err(anyhow!("Failed to retrieve memory info"));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let total_memory = output_str
            .lines()
            .filter(|line| line.starts_with("Mem:"))
            .map(|line| line.split_whitespace().nth(1).unwrap_or("0"))
            .next()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);

        let free_memory = output_str
            .lines()
            .filter(|line| line.starts_with("Mem:"))
            .map(|line| line.split_whitespace().nth(3).unwrap_or("0"))
            .next()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);

        Ok(SystemInfo {
            os_name: "Linux".to_string(),
            architecture: "x86_64".to_string(),
            cpu: 0.0, // Placeholder for CPU info
            ram_total: total_memory as i64,
            ram_used: (total_memory - free_memory) as i64,
            ram_free: free_memory as i64,
            disk_total: 0,
            disk_free: 0,
            disk_health: "Healthy".to_string(),
            network_throughput: 0,
            ping_latency: None,
            network_interfaces: None,
            ip_address: None,
        })
    }
}
