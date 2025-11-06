use anyhow::Result;
use serde_json::json;
use sysinfo::System;
use local_ip_address::local_ip;
use std::process::Command;

#[cfg(windows)]
mod windows {
    use super::*;

    pub fn get_system_info() -> Result<serde_json::Value> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let hostname = sys.host_name().unwrap_or_default();
        let os_name = sys.name().unwrap_or_default();
        let os_version = sys.os_version().unwrap_or_default();
        let kernel_version = sys.kernel_version().unwrap_or_default();
        let cpu_count = sys.cpus().len();
        let cpu_brand = sys.cpus().get(0).map(|c| c.brand().to_string()).unwrap_or_default();
        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let free_memory = sys.free_memory();
        let device_model = get_device_model();
        let serial_number = get_serial_number();
        let ip_address = local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "0.0.0.0".to_string());
        let device_type = std::env::consts::OS;
        let architecture = std::env::consts::ARCH;

        Ok(json!({
            "system_info": {
                "hostname": hostname,
                "os_name": os_name,
                "os_version": os_version,
                "kernel_version": kernel_version,
                "cpu_brand": cpu_brand,
                "cpu_count": cpu_count,
                "total_memory": total_memory,
                "used_memory": used_memory,
                "free_memory": free_memory,
                "device_type": device_type,
                "device_model": device_model,
                "serial_number": serial_number,
                "architecture": architecture,
                "ip_address": ip_address
            }
        }))
    }

    fn get_device_model() -> String {
        Command::new("wmic")
            .args(["computersystem", "get", "model"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    o.stdout.lines().nth(1).map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn get_serial_number() -> String {
        Command::new("wmic")
            .args(["bios", "get", "serialnumber"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    o.stdout.lines().nth(1).map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[cfg(unix)]
mod unix {
    use super::*;

    pub fn get_system_info() -> Result<serde_json::Value> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let hostname = sys.host_name().unwrap_or_default();
        let os_name = sys.name().unwrap_or_default();
        let os_version = sys.os_version().unwrap_or_default();
        let kernel_version = sys.kernel_version().unwrap_or_default();
        let cpu_count = sys.cpus().len();
        let cpu_brand = sys.cpus().get(0).map(|c| c.brand().to_string()).unwrap_or_default();
        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let device_model = get_device_model();
        let serial_number = get_serial_number();
        let ip_address = local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "0.0.0.0".to_string());
        let device_type = std::env::consts::OS;
        let architecture = std::env::consts::ARCH;

        Ok(json!({
            "system_info": {
                "hostname": hostname,
                "os_name": os_name,
                "os_version": os_version,
                "kernel_version": kernel_version,
                "cpu_brand": cpu_brand,
                "cpu_count": cpu_count,
                "total_memory": total_memory,
                "used_memory": used_memory,
                "free_memory": total_memory.checked_sub(used_memory).unwrap_or(0),
                "device_type": device_type,
                "device_model": device_model,
                "serial_number": serial_number,
                "architecture": architecture,
                "ip_address": ip_address
            }
        }))
    }

    fn get_device_model() -> String {
        Command::new("cat")
            .arg("/sys/devices/virtual/dmi/id/product_name")
            .output()
            .ok()
            .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None })
            .or_else(|| {
                Command::new("sysctl").args(["-n", "hw.model"]).output().ok()
                    .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None })
            })
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn get_serial_number() -> String {
        Command::new("cat")
            .arg("/sys/devices/virtual/dmi/id/product_serial")
            .output()
            .ok()
            .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None })
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[cfg(windows)]
pub use windows::get_system_info;

#[cfg(unix)]
pub use unix::get_system_info;
