use std::process::Command;
use anyhow::{Result, anyhow};
use serde_json::json;
use log::{info, error};
use crate::models::{DeviceInfo, SystemInfo}; // Add imports for DeviceInfo and SystemInfo

#[cfg(windows)]
mod windows {
    use super::*;

    pub fn get_system_info() -> Result<DeviceInfo> {
        info!("Retrieving system information for Windows...");

        let serial_number = get_serial_number()?;
        let os_info = get_os_info()?;
        let cpu_info = get_cpu_info()?;
        let memory_info = get_memory_info()?;

        let system_info = SystemInfo {
            os_name: os_info,
            architecture: "x86_64".to_string(), // Update as necessary for architecture
            cpu: cpu_info,
            ram_total: memory_info.total_memory,
            ram_used: memory_info.used_memory,
            ram_free: memory_info.free_memory,
            disk_total: 0, // Could be fetched from another system command
            disk_free: 0,  // Same as above
            disk_health: "Healthy".to_string(), // Replace with actual logic if needed
            network_throughput: 0, // Placeholder for network throughput
            ping_latency: None, // Placeholder
            network_interfaces: None, // Placeholder
            ip_address: None, // Placeholder
        };

        Ok(DeviceInfo {
            system_info,
            device_type: None, // Set if available
            device_model: None, // Set if available
        })
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

    fn get_memory_info() -> Result<SystemInfo> {
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

        Ok(SystemInfo {
            os_name: "Windows".to_string(), // For simplicity; can be dynamic
            architecture: "x86_64".to_string(),
            cpu: 0.0, // Placeholder, will fill later
            ram_total: total_memory as i64,
            ram_used: (total_memory - free_memory) as i64,
            ram_free: free_memory as i64,
            disk_total: 0, // Placeholder for disk info
            disk_free: 0,  // Same as above
            disk_health: "Healthy".to_string(),
            network_throughput: 0,
            ping_latency: None,
            network_interfaces: None,
            ip_address: None,
        })
    }
}

#[cfg(unix)]
mod unix {
    use super::*;

    pub fn get_system_info() -> Result<DeviceInfo> {
        info!("Retrieving system information for Unix...");

        let serial_number = get_serial_number()?;
        let os_info = get_os_info()?;
        let cpu_info = get_cpu_info()?;
        let memory_info = get_memory_info()?;

        let system_info = SystemInfo {
            os_name: os_info,
            architecture: "x86_64".to_string(), // Update as necessary for architecture
            cpu: cpu_info,
            ram_total: memory_info.total_memory,
            ram_used: memory_info.used_memory,
            ram_free: memory_info.free_memory,
            disk_total: 0, // Could be fetched from another system command
            disk_free: 0,  // Same as above
            disk_health: "Healthy".to_string(), // Replace with actual logic if needed
            network_throughput: 0, // Placeholder for network throughput
            ping_latency: None, // Placeholder
            network_interfaces: None, // Placeholder
            ip_address: None, // Placeholder
        };

        Ok(DeviceInfo {
            system_info,
            device_type: None, // Set if available
            device_model: None, // Set if available
        })
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
        let output = Command::new("top")
            .args(&["-bn1"])
            .output()
            .map_err(|e| anyhow!("Failed to execute top command for CPU info: {}", e))?;

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
            cpu: 0.0, // Placeholder for CPU usage
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
