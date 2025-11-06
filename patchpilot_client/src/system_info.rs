use anyhow::Result;
use serde::{Serialize, Deserialize};
use sysinfo::{System, SystemExt, CpuExt, DiskExt, NetworkExt};
use local_ip_address::local_ip;
use get_if_addrs::get_if_addrs;
use std::process::Command;

/// System info struct for server payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os_name: String,
    pub architecture: String,
    pub cpu: f32,            // CPU usage percentage
    pub ram_total: u64,      // MB
    pub ram_used: u64,       // MB
    pub ram_free: u64,       // MB
    pub disk_total: u64,     // MB
    pub disk_free: u64,      // MB
    pub disk_health: String,
    pub network_throughput: u64,
    pub ping_latency: Option<f32>,
    pub network_interfaces: Option<String>,
    pub ip_address: Option<String>,
}

/// Returns system info for client
pub fn get_system_info() -> Result<SystemInfo> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu = sys.global_cpu_info().cpu_usage();

    let ram_total = sys.total_memory() / 1024; // MB
    let ram_used = sys.used_memory() / 1024;
    let ram_free = ram_total - ram_used;

    let disk_total = sys.disks().iter().map(|d| d.total_space()).sum::<u64>() / 1024 / 1024;
    let disk_free = sys.disks().iter().map(|d| d.available_space()).sum::<u64>() / 1024 / 1024;

    let disk_health = "Good".to_string(); // placeholder, could integrate SMART checks

    let network_throughput = sys.networks()
        .iter()
        .map(|(_, data)| data.received() + data.transmitted())
        .sum::<u64>();

    let ping_latency = ping("8.8.8.8");

    let network_interfaces = get_interfaces().ok();

    let ip_address = local_ip().ok().map(|ip| ip.to_string());

    Ok(SystemInfo {
        os_name: sys.name().unwrap_or_else(|| "Unknown".into()),
        architecture: std::env::consts::ARCH.to_string(),
        cpu,
        ram_total,
        ram_used,
        ram_free,
        disk_total,
        disk_free,
        disk_health,
        network_throughput,
        ping_latency,
        network_interfaces,
        ip_address,
    })
}

/// List network interfaces as JSON string
fn get_interfaces() -> Result<String> {
    let mut list = vec![];
    for iface in get_if_addrs()? {
        list.push(serde_json::json!({
            "name": iface.name,
            "ip": iface.ip().to_string(),
            "loopback": iface.is_loopback()
        }));
    }
    Ok(serde_json::to_string(&list)?)
}

/// Ping an IP and return latency in milliseconds
fn ping(host: &str) -> Option<f32> {
    #[cfg(unix)]
    {
        let output = Command::new("ping")
            .args(["-c", "1", host])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(pos) = line.find("time=") {
                let time_str = &line[pos + 5..];
                if let Some(ms_str) = time_str.split_whitespace().next() {
                    return ms_str.parse::<f32>().ok();
                }
            }
        }
    }

    #[cfg(windows)]
    {
        let output = Command::new("ping")
            .args([host, "-n", "1"])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains("Average =") {
                let ms_str = line.split("Average =").nth(1)?.replace("ms", "").trim().to_string();
                return ms_str.parse::<f32>().ok();
            }
        }
    }

    None
}

#[cfg(windows)]
mod windows {
    use super::*;
    use serde_json::json;
    use std::process::Command;

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
    use std::process::Command;

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
