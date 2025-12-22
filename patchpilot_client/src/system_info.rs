use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use local_ip_address::local_ip;
use sysinfo::{System, Disks, Networks};

/// Default refresh interval (seconds)
static SYSTEM_INFO_REFRESH_SECS: AtomicU64 = AtomicU64::new(10);

pub fn set_system_info_refresh_secs(secs: u64) {
    SYSTEM_INFO_REFRESH_SECS.store(secs.max(1), Ordering::SeqCst);
}

pub fn get_system_info_refresh_secs() -> u64 {
    SYSTEM_INFO_REFRESH_SECS.load(Ordering::SeqCst)
}

#[cfg(any(unix, target_os = "macos"))]
const DEVICE_ID_FILE: &str = "/opt/patchpilot_client/device_id.txt";
#[cfg(windows)]
const DEVICE_ID_FILE: &str = "C:\\ProgramData\\PatchPilot\\device_id.txt";

#[cfg(any(unix, target_os = "macos"))]
const SERVER_URL_FILE: &str = "/opt/patchpilot_client/server_url.txt";
#[cfg(windows)]
const SERVER_URL_FILE: &str = "C:\\ProgramData\\PatchPilot\\server_url.txt";

/// Matches server-side expectations (extra fields are ignored server-side)
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
    pub network_interfaces: Option<Vec<String>>,
    pub ip_address: Option<String>,

    pub device_type: String,
    pub device_model: String,
}

impl SystemInfo {
    /// Blocking system probe (safe to call inside spawn_blocking)
    pub fn gather_blocking() -> Self {
        let sys = System::new_all();

        // ---- Host / OS ----
        let hostname =
            System::host_name().unwrap_or_else(|| "unknown".to_string());
        let os_name =
            System::long_os_version().unwrap_or_else(|| "unknown".to_string());
        let architecture = std::env::consts::ARCH.to_string();

        // ---- CPU ----
        let cpus = sys.cpus();
        let cpu_count = cpus.len() as i32;

        let cpu_brand = cpus
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_default();

        let cpu_usage = if cpu_count > 0 {
            cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpu_count as f32
        } else {
            0.0
        };

        // ---- Memory ----
        let ram_total = sys.total_memory() as i64;
        let ram_used = sys.used_memory() as i64;

        // ---- Disks ----
        let disks = Disks::new_with_refreshed_list();
        let mut disk_total = 0i64;
        let mut disk_free = 0i64;

        for disk in disks.iter() {
            disk_total += disk.total_space() as i64;
            disk_free += disk.available_space() as i64;
        }

        // ---- Network ----
        let networks = Networks::new_with_refreshed_list();

        let network_interfaces = Some(
            networks.iter().map(|(name, _)| name.clone()).collect(),
        );

        let network_throughput: i64 = networks
            .iter()
            .map(|(_, data)| {
                (data.received() + data.transmitted()) as i64
            })
            .sum();

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
            disk_health: "unknown".to_string(),

            network_throughput,
            network_interfaces,
            ip_address,

            device_type: String::new(),
            device_model: String::new(),
        }
    }
}

/// Async cached system info service
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
            if let (Some(ts), Some(info)) = (*last, &*cache) {
                if ts.elapsed() < Duration::from_secs(refresh_secs) {
                    return Ok(info.clone());
                }
            }
        }

        let info = tokio::task::spawn_blocking(SystemInfo::gather_blocking)
            .await
            .context("spawn_blocking failed")?;

        *self.cache.write().await = Some(info.clone());
        *self.last.write().await = Some(std::time::Instant::now());

        Ok(info)
    }
}

// ---- Helpers ----

pub fn get_system_info() -> SystemInfo {
    SystemInfo::gather_blocking()
}

pub async fn read_server_url() -> Result<String> {
    let path = PathBuf::from(SERVER_URL_FILE);
    let raw = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("Failed to read server URL from {:?}", path))?;
    Ok(raw.trim().to_string())
}

pub fn get_local_device_id() -> Option<String> {
    fs::read_to_string(DEVICE_ID_FILE)
        .ok()
        .map(|s| s.trim().to_string())
}

pub fn write_local_device_id(device_id: &str) -> Result<()> {
    fs::write(DEVICE_ID_FILE, device_id)
        .context("Failed to write local device_id")
}

pub fn get_device_info_basic() -> (String, String) {
    let si = get_system_info();
    (si.device_type, si.device_model)
}
