use anyhow::{Result, anyhow};
use serde_json::json;
use std::process::Command;
use local_ip_address::local_ip;

#[cfg(windows)]
#[allow(dead_code)]
mod windows {
    use super::*;

    pub fn get_wifi_info() -> Result<serde_json::Value> {
        // List all Wi-Fi networks
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

        // Check currently connected SSID
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

    // Add to top-level network_info
    pub fn get_network_info() -> Result<serde_json::Value> {
        let ip_address = local_ip()?.to_string();
        let wifi_info = get_wifi_info().unwrap_or(json!({}));
        Ok(json!({ "ip_address": ip_address, "wifi": wifi_info }))
    }
}

#[cfg(unix)]
#[allow(dead_code)]
mod unix {
    use super::*;
    use std::process::Command;
    use sysinfo::System;
    use std::path::Path;

    pub fn get_wifi_info() -> Result<serde_json::Value> {
        // Using `nmcli` for Linux
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
            if parts.len() < 3 { continue; }

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
        let ip_address = local_ip()?.to_string();
        let wifi_info = get_wifi_info().unwrap_or(json!({}));
        Ok(json!({ "ip_address": ip_address, "wifi": wifi_info }))
    }
}

// --- Top-level forwarders ---
pub fn get_network_info() -> Result<serde_json::Value> {
    #[cfg(windows)] { windows::get_network_info() }
    #[cfg(unix)] { unix::get_network_info() }
}
