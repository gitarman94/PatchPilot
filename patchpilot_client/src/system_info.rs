use anyhow::{Result, anyhow};
use serde_json::json;
use std::process::Command;
use std::env;
use sysinfo::{System, Disk, NetworkData};

#[cfg(windows)]
#[allow(dead_code)]
mod windows {
    use super::*;

    pub fn get_serial_number() -> Result<String> {
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
        let output = Command::new("systeminfo")
            .args(["/fo", "CSV"])
            .output()
            .map_err(|e| anyhow!("Failed to execute systeminfo: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Failed to retrieve OS info"));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn get_cpu_info() -> Result<f32> {
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
            .trim();

        Ok(cpu_str.parse::<f32>().unwrap_or(0.0))
    }

    pub fn get_memory_info() -> Result<serde_json::Value> {
        // Use sysinfo below for better accuracy; this Windows fallback is kept.
        let output = Command::new("systeminfo")
            .args(["/fo", "CSV"])
            .output()
            .map_err(|e| anyhow!("Failed to execute systeminfo: {}", e))?;

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

        Ok(json!({ "total": total_memory, "free": free_memory, "used": total_memory.saturating_sub(free_memory) }))
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
#[allow(dead_code)]
mod unix {
    use super::*;
    use std::path::Path;

    pub fn get_serial_number() -> Result<String> {
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
        // Use sysinfo instead of parsing free manually
        let mut sys = System::new_all();
        sys.refresh_memory();

        let total = sys.total_memory() as i64;
        let free = sys.available_memory() as i64;
        let used = total - free;

        Ok(json!({ "total": total, "free": free, "used": used }))
    }

    pub fn get_device_type() -> String {
        if Path::new("/sys/class/power_supply/BAT0").exists() {
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
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
            _ => "Unknown Model".to_string(),
        }
    }
}

// --- Top-level forwarders ---
pub fn get_device_type() -> String {
    #[cfg(windows)] { windows::get_device_type() }
    #[cfg(unix)] { unix::get_device_type() }
}

pub fn get_device_model() -> String {
    #[cfg(windows)] { windows::get_device_model() }
    #[cfg(unix)] { unix::get_device_model() }
}

// --- Helper to build JSON ---
fn build_system_info(
    serial_number: String,
    os_info: String,
    architecture: String,
    cpu: f32,
    memory: &serde_json::Value,
    disk_total: i64,
    disk_free: i64,
    disk_health: String,
    network_throughput: i64,
    ping_latency: Option<f32>,
    device_type: String,
    device_model: String,
) -> serde_json::Value {
    json!({
        "system_info": {
            "os_name": os_info,
            "architecture": architecture,
            "cpu": cpu,
            "ram_total": memory["total"],
            "ram_used": memory["used"],
            "ram_free": memory["free"],
            "disk_total": disk_total,
            "disk_free": disk_free,
            "disk_health": disk_health,
            "network_throughput": network_throughput,
            "ping_latency": ping_latency
        },
        "device_type": device_type,
        "device_model": device_model
    })
}

// --- Main entry ---
pub fn get_system_info() -> Result<serde_json::Value> {
    // Create sysinfo early for disk + network
    let mut sys = System::new_all();
    sys.refresh_all();

    // Architecture
    let architecture = env::consts::ARCH.to_string();

    // Disk total & free
    let disk = sys.disks().first();
    let (disk_total, disk_free, disk_health) = if let Some(d) = disk {
        (d.total_space() as i64,
         d.available_space() as i64,
         "Healthy".to_string())
    } else {
        (0, 0, "Unknown".to_string())
    };

    let network_throughput = sys.network_interfaces()
        .iter()
        .map(|(_, data)| data.received() + data.transmitted())
        .sum::<u64>() as i64;

    let ping_latency: Option<f32> = None; // Optionally implement ping

    #[cfg(windows)] {
        let serial_number = windows::get_serial_number()?;
        let os_info = windows::get_os_info()?;
        let cpu = windows::get_cpu_info()?;
        let memory = windows::get_memory_info()?;
        let device_type = windows::get_device_type();
        let device_model = windows::get_device_model();

        Ok(build_system_info(
            serial_number,
            os_info,
            architecture,
            cpu,
            &memory,
            disk_total,
            disk_free,
            disk_health,
            network_throughput,
            ping_latency,
            device_type,
            device_model,
        ))
    }

    #[cfg(unix)] {
        let serial_number = unix::get_serial_number()?;
        let os_info = unix::get_os_info()?;
        let cpu = unix::get_cpu_info()?;
        let memory = unix::get_memory_info()?;
        let device_type = unix::get_device_type();
        let device_model = unix::get_device_model();

        Ok(build_system_info(
            serial_number,
            os_info,
            architecture,
            cpu,
            &memory,
            disk_total,
            disk_free,
            disk_health,
            network_throughput,
            ping_latency,
            device_type,
            device_model,
        ))
    }
}
