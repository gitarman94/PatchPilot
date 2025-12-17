use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, time::Duration};
use local_ip_address::local_ip;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use sysinfo::System;
use std::path::PathBuf;

// Intervals (defaults)
const ADOPTION_CHECK_INTERVAL: u64 = 10;
const DEFAULT_SYSTEM_UPDATE_INTERVAL: u64 = 600;

// Path constants
#[cfg(any(unix, target_os = "macos"))]
const DEVICE_ID_FILE: &str = "/opt/patchpilot_client/device_id.txt";
#[cfg(windows)]
const DEVICE_ID_FILE: &str = "C:\\ProgramData\\PatchPilot\\device_id.txt";

// Refresh interval
static SYSTEM_INFO_REFRESH_SECS: AtomicU64 = AtomicU64::new(10);

/// Set the refresh interval for cached system info
pub fn set_system_info_refresh_secs(secs: u64) {
    SYSTEM_INFO_REFRESH_SECS.store(if secs == 0 { 10 } else { secs }, Ordering::SeqCst);
}

pub fn get_system_info_refresh_secs() -> u64 {
    SYSTEM_INFO_REFRESH_SECS.load(Ordering::SeqCst)
}

// Command spec and types
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CommandSpec {
    #[serde(rename = "shell")]
    Shell { command: String, timeout_secs: Option<u64> },
    #[serde(rename = "script")]
    Script { name: String, args: Option<Vec<String>>, timeout_secs: Option<u64> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerCommand {
    pub id: String,
    pub spec: CommandSpec,
    pub created_at: Option<String>,
    pub run_as_root: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CommandResult {
    pub id: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_secs: f64,
    pub success: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SystemInfo {
    pub hostname: String,
    pub os_name: String,
    pub architecture: String,
    pub cpu_usage: f32,
    pub cpu_count: i32,
    pub cpu_brand: String,
    pub ram_total: i64,
    pub ram_used: i64,
    pub disk_total: i64,
    pub disk_free: i64,
    pub disk_health: String,
    pub network_throughput: i64,
    pub ping_latency: Option<f32>,
    pub network_interfaces: Option<String>,
    pub ip_address: Option<String>,
    pub device_type: String,
    pub device_model: String,
}

// Blocking gather
impl SystemInfo {
    pub fn gather_blocking() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let hostname =
            sysinfo::System::host_name().unwrap_or_else(|| "unknown".to_string());

        let os_name =
            sysinfo::System::long_os_version().unwrap_or_else(|| "unknown".to_string());

        let architecture = std::env::consts::ARCH.to_string();

        let cpus = sys.cpus();
        let cpu_count = cpus.len() as i32;
        let cpu_brand = cpus.first().map(|c| c.brand().to_string()).unwrap_or_default();
        let cpu_usage = if cpu_count == 0 {
            0.0
        } else {
            cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpu_count as f32
        };

        let ram_total = sys.total_memory() as i64;
        let ram_used = sys.used_memory() as i64;

        let mut disk_total: i64 = 0;
        let mut disk_free: i64 = 0;
        let disks = sysinfo::Disks::new_with_refreshed_list();
        for disk in disks.iter() {
            disk_total += disk.total_space() as i64;
            disk_free += disk.available_space() as i64;
        }

        let ip_address = local_ip().ok().map(|ip| ip.to_string());

        SystemInfo {
            hostname,
            os_name,
            architecture,
            cpu_usage,
            cpu_count,
            cpu_brand,
            ram_total,
            ram_used,
            disk_total,
            disk_free,
            disk_health: String::new(),
            network_throughput: 0,
            ping_latency: None,
            network_interfaces: None,
            ip_address,
            device_type: String::new(),
            device_model: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct SystemInfoService {
    cache: Arc<tokio::sync::RwLock<Option<SystemInfo>>>,
    last: Arc<tokio::sync::RwLock<Option<std::time::Instant>>>,
}

impl Default for SystemInfoService {
    fn default() -> Self {
        Self {
            cache: Arc::new(tokio::sync::RwLock::new(None)),
            last: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }
}

impl SystemInfoService {
    pub async fn get_system_info_async(&self) -> Result<SystemInfo> {
        let refresh_secs = get_system_info_refresh_secs();
        {
            let last = self.last.read().await;
            let cache = self.cache.read().await;
            if let (Some(ts), Some(si)) = (*last, &*cache) {
                if ts.elapsed() < Duration::from_secs(refresh_secs) {
                    return Ok(si.clone());
                }
            }
        }

        let mut last = self.last.write().await;
        let mut cache = self.cache.write().await;

        if let (Some(ts), Some(si)) = (*last, &*cache) {
            if ts.elapsed() < Duration::from_secs(refresh_secs) {
                return Ok(si.clone());
            }
        }

        let info = tokio::task::spawn_blocking(move || SystemInfo::gather_blocking())
            .await
            .context("spawn_blocking failed")?;

        *cache = Some(info.clone());
        *last = Some(std::time::Instant::now());
        Ok(info)
    }
}

// Convenience function
pub fn get_system_info() -> SystemInfo {
    SystemInfo::gather_blocking()
}

// Local device helpers
pub fn get_local_device_id() -> Option<String> {
    fs::read_to_string(DEVICE_ID_FILE).ok().map(|s| s.trim().to_string())
}

pub fn write_local_device_id(device_id: &str) -> Result<()> {
    fs::write(DEVICE_ID_FILE, device_id).context("Failed to write local device_id")
}

pub async fn read_server_url() -> Result<String> {
    let base_dir = crate::get_base_dir();
    let url_file = PathBuf::from(base_dir).join("server_url.txt");
    let url = fs::read_to_string(url_file)?;
    Ok(url.trim().to_string())
}


pub fn get_device_info_basic() -> (String, String) {
    let si = get_system_info();
    (si.device_type, si.device_model)
}
