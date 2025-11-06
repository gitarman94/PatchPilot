use anyhow::{anyhow, Result};
use serde_json::json;
use sysinfo::System;
use local_ip_address::local_ip;
use std::process::Command;

#[cfg(windows)]
mod windows {
    use super::*;

    pub fn get_wifi_info() -> Result<serde_json::Value> {
        let output = Command::new("netsh")
            .args(["wlan", "show", "networks", "mode=Bssid"])
            .output()
            .map_err(|e| anyhow!("Failed to execute netsh wlan: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Failed to retrieve Wi-Fi networks"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut networks = vec![];
        let mut current = serde_json::Map::new();

        for line in stdout.lines() {
            let line = line.trim();
            if line.starts_with("SSID ") {
                if !current.is_empty() {
                    networks.push(json!(current));
                    current = serde_json::Map::new();
                }
                let ssid = line.split(':').nth(1).unwrap_or("").trim();
                current.insert("ssid".to_string(), json!(ssid));
            } else if line.starts_with("Signal") {
                let signal = line.split(':').nth(1).unwrap_or("").trim();
                current.insert("signal".to_string(), json!(signal));
            } else if line.starts_with("BSSID") {
                let bssid = line.split(':').nth(1).unwrap_or("").trim();
                current.insert("bssid".to_string(), json!(bssid));
            } else if line.starts_with("Authentication") {
                let auth = line.split(':').nth(1).unwrap_or("").trim();
                current.insert("auth".to_string(), json!(auth));
            }
        }

        if !current.is_empty() {
            networks.push(json!(current));
        }

        let connected_output = Command::new("netsh")
            .args(["wlan", "show", "interfaces"])
            .output()
            .map_err(|e| anyhow!("Failed to execute netsh wlan show interfaces: {}", e))?;

        let connected_ssid = if connected_output.status.success() {
            String::from_utf8_lossy(&connected_output.stdout)
                .lines()
                .find(|line| line.trim_start().starts_with("SSID"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        } else {
            String::new()
        };

        Ok(json!({
            "connected_ssid": connected_ssid,
            "networks": networks
        }))
    }

    pub fn get_network_info() -> Result<serde_json::Value> {
        let ip_address = local_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|_| "0.0.0.0".to_string());
        let wifi_info = get_wifi_info().unwrap_or(json!({}));
        Ok(json!({
            "ip_address": ip_address,
            "wifi": wifi_info
        }))
    }

    pub fn get_system_info() -> Result<serde_json::Value> {
        let mut sys = System::new_all();
        sys.refresh_all();

        // Fixed: Use associated functions (not methods)
        let hostname = System::host_name().unwrap_or_else(|| "undefined".to_string());
        let os_name = System::name().unwrap_or_else(|| "undefined".to_string());
        let os_version = System::os_version().unwrap_or_else(|| "undefined".to_string());
        let kernel_version = System::kernel_version().unwrap_or_else(|| "undefined".to_string());

        let cpu_count = sys.cpus().len();
        let cpu_brand = sys
            .cpus()
            .get(0)
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "undefined".to_string());

        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let free_memory = sys.free_memory();

        let device_model = get_device_model();
        let serial_number = get_serial_number();

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
                "device_type": "windows",
                "device_model": device_model,
                "serial_number": serial_number,
                "architecture": std::env::consts::ARCH,
                "ip_address": local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "0.0.0.0".to_string())
            }
        }))
    }

    fn get_device_model() -> String {
        if let Ok(output) = Command::new("wmic")
            .args(["computersystem", "get", "model"])
            .output()
        {
            if output.status.success() {
                let lines: Vec<_> = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .skip(1)
                    .collect();
                if let Some(model) = lines.first() {
                    return model.trim().to_string();
                }
            }
        }
        "generic".to_string()
    }

    fn get_serial_number() -> String {
        if let Ok(output) = Command::new("wmic")
            .args(["bios", "get", "serialnumber"])
            .output()
        {
            if output.status.success() {
                let lines: Vec<_> = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .skip(1)
                    .collect();
                if let Some(serial) = lines.first() {
                    return serial.trim().to_string();
                }
            }
        }
        "undefined".to_string()
    }
}

#[cfg(unix)]
mod unix {
    use super::*;

    pub fn get_wifi_info() -> Result<serde_json::Value> {
        let output = Command::new("nmcli")
            .args(["-t", "-f", "SSID,SIGNAL,ACTIVE", "dev", "wifi"])
            .output()
            .map_err(|e| anyhow!("Failed to execute nmcli: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Failed to retrieve Wi-Fi networks"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut networks = vec![];
        let mut connected_ssid = "".to_string();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 3 {
                continue;
            }

            let ssid = parts[0].to_string();
            let signal = parts[1].to_string();
            let active = parts[2] == "yes";

            if active {
                connected_ssid = ssid.clone();
            }

            networks.push(json!({
                "ssid": ssid,
                "signal": signal,
                "connected": active
            }));
        }

        Ok(json!({
            "connected_ssid": connected_ssid,
            "networks": networks
        }))
    }

    pub fn get_network_info() -> Result<serde_json::Value> {
        let ip_address = local_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|_| "0.0.0.0".to_string());
        let wifi_info = get_wifi_info().unwrap_or(json!({}));
        Ok(json!({
            "ip_address": ip_address,
            "wifi": wifi_info
        }))
    }

    pub fn get_system_info() -> Result<serde_json::Value> {
        let mut sys = System::new_all();
        sys.refresh_all();

        // Fix: use associated functions for system-level data
        let hostname = System::host_name().unwrap_or_else(|| "undefined".to_string());
        let os_name = System::name().unwrap_or_else(|| "undefined".to_string());
        let os_version = System::os_version().unwrap_or_else(|| "undefined".to_string());
        let kernel_version = System::kernel_version().unwrap_or_else(|| "undefined".to_string());

        let cpu_count = sys.cpus().len();
        let cpu_brand = sys
            .cpus()
            .get(0)
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "undefined".to_string());

        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();

        let device_model = get_device_model();
        let serial_number = get_serial_number();

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
                "device_type": "unix",
                "device_model": device_model,
                "serial_number": serial_number,
                "architecture": std::env::consts::ARCH,
                "ip_address": local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "0.0.0.0".to_string())
            }
        }))
    }

    fn get_device_model() -> String {
        if let Ok(output) = Command::new("cat")
            .arg("/sys/devices/virtual/dmi/id/product_name")
            .output()
        {
            if output.status.success() {
                let model = String::from_utf8_lossy(&output.stdout);
                return model.trim().to_string();
            }
        }

        if let Ok(output) = Command::new("sysctl")
            .args(["-n", "hw.model"])
            .output()
        {
            if output.status.success() {
                let model = String::from_utf8_lossy(&output.stdout);
                return model.trim().to_string();
            }
        }

        "generic".to_string()
    }

    fn get_serial_number() -> String {
        if let Ok(output) = Command::new("cat")
            .arg("/sys/devices/virtual/dmi/id/product_serial")
            .output()
        {
            if output.status.success() {
                let serial = String::from_utf8_lossy(&output.stdout);
                return serial.trim().to_string();
            }
        }
        "undefined".to_string()
    }
}

#[cfg(windows)]
pub use windows::{get_system_info, get_network_info, get_wifi_info};

#[cfg(unix)]
pub use unix::{get_system_info, get_network_info, get_wifi_info};

