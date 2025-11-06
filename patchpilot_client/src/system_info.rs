use anyhow::Result;
use serde_json::json;
use std::process::Command;
use local_ip_address::local_ip;
use std::env;

/// System info struct for server payload
#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemInfo {
    pub os_name: String,
    pub architecture: String,
    pub cpu: f32,
    pub ram_total: u64,
    pub ram_used: u64,
    pub ram_free: u64,
    pub disk_total: u64,
    pub disk_free: u64,
    pub disk_health: String,
    pub network_throughput: u64,
    pub ping_latency: Option<f32>,
    pub network_interfaces: Option<String>,
    pub ip_address: Option<String>,
}

/// Returns system info for client
pub fn get_system_info() -> Result<SystemInfo> {
    Ok(SystemInfo {
        os_name: env::consts::OS.to_string(),
        architecture: env::consts::ARCH.to_string(),
        cpu: 0.5,                 // placeholder: add real CPU metrics if needed
        ram_total: 16 * 1024,     // MB
        ram_used: 8 * 1024,
        ram_free: 8 * 1024,
        disk_total: 256 * 1024,   // MB
        disk_free: 128 * 1024,
        disk_health: "Good".to_string(),
        network_throughput: 1000, // placeholder
        ping_latency: None,
        network_interfaces: None,
        ip_address: local_ip().ok().map(|ip| ip.to_string()),
    })
}

#[cfg(windows)]
mod windows {
    use super::*;
    use serde_json::json;

    pub fn get_wifi_info() -> Result<serde_json::Value> {
        let output = Command::new("netsh")
            .args(["wlan", "show", "networks", "mode=Bssid"])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to execute netsh wlan: {}", e))?;

        if !output.status.success() {
            return Ok(json!({
                "connected_ssid": null,
                "networks": []
            }));
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
                let ssid = line.split(':').nth(1).map(|s| s.trim().to_string());
                current.insert("ssid".to_string(), json!(ssid));
            } else if line.starts_with("Signal") {
                let signal = line.split(':').nth(1).map(|s| s.trim().to_string());
                current.insert("signal".to_string(), json!(signal));
            } else if line.starts_with("BSSID") {
                let bssid = line.split(':').nth(1).map(|s| s.trim().to_string());
                current.insert("bssid".to_string(), json!(bssid));
            } else if line.starts_with("Authentication") {
                let auth = line.split(':').nth(1).map(|s| s.trim().to_string());
                current.insert("auth".to_string(), json!(auth));
            }
        }

        if !current.is_empty() {
            networks.push(json!(current));
        }

        let connected_output = Command::new("netsh")
            .args(["wlan", "show", "interfaces"])
            .output()
            .ok();

        let connected_ssid = connected_output
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout).ok()
                        .and_then(|s| {
                            s.lines()
                             .find(|l| l.trim_start().starts_with("SSID"))
                             .and_then(|l| l.split(':').nth(1))
                             .map(|s| s.trim().to_string())
                        })
                } else {
                    None
                }
            });

        Ok(json!({
            "connected_ssid": connected_ssid,
            "networks": networks
        }))
    }

    pub fn get_network_info() -> Result<serde_json::Value> {
        let ip_address = local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "0.0.0.0".to_string());
        let wifi_info = get_wifi_info().unwrap_or(json!({
            "connected_ssid": null,
            "networks": []
        }));
        Ok(json!({
            "ip_address": ip_address,
            "wifi": wifi_info
        }))
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use serde_json::json;

    pub fn get_wifi_info() -> Result<serde_json::Value> {
        let output = Command::new("nmcli")
            .args(["-t", "-f", "SSID,SIGNAL,ACTIVE,BSSID,SECURITY", "dev", "wifi"])
            .output()
            .ok();

        if output.is_none() || !output.as_ref().unwrap().status.success() {
            return Ok(json!({
                "connected_ssid": null,
                "networks": []
            }));
        }

        let output = output.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut networks = vec![];
        let mut connected_ssid: Option<String> = None;

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 5 {
                continue;
            }

            let ssid = Some(parts[0].to_string());
            let signal = Some(parts[1].to_string());
            let active = parts[2] == "yes";
            let bssid = Some(parts[3].to_string());
            let auth = Some(parts[4].to_string());

            if active {
                connected_ssid = ssid.clone();
            }

            networks.push(json!({
                "ssid": ssid,
                "signal": signal,
                "bssid": bssid,
                "auth": auth,
                "connected": active
            }));
        }

        Ok(json!({
            "connected_ssid": connected_ssid,
            "networks": networks
        }))
    }

    pub fn get_network_info() -> Result<serde_json::Value> {
        let ip_address = local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "0.0.0.0".to_string());
        let wifi_info = get_wifi_info().unwrap_or(json!({
            "connected_ssid": null,
            "networks": []
        }));
        Ok(json!({
            "ip_address": ip_address,
            "wifi": wifi_info
        }))
    }
}

#[cfg(windows)]
pub use windows::{get_network_info, get_wifi_info};

#[cfg(unix)]
pub use unix::{get_network_info, get_wifi_info};
