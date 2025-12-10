use std::collections::HashMap;
use std::fs;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use std::process::Command;

use lazy_static::lazy_static;
use local_ip_address::local_ip;
use serde::{Deserialize, Serialize};
use sysinfo::{
    CpuRefreshKind, DiskRefreshKind, Disks, MemoryRefreshKind, Networks, RefreshKind, System,
};

/// Small telemetry/metrics collector (atomic counters). Exposed as JSON for easy scraping.
pub struct Telemetry {
    pub refresh_count: AtomicU64,
    pub rebuild_count: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub heartbeats_sent: AtomicU64,
}

impl Telemetry {
    fn new() -> Self {
        Self {
            refresh_count: AtomicU64::new(0),
            rebuild_count: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            heartbeats_sent: AtomicU64::new(0),
        }
    }

    pub fn incr_refresh(&self) {
        self.refresh_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_rebuild(&self) {
        self.rebuild_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_bytes_sent(&self, n: u64) {
        self.bytes_sent.fetch_add(n, Ordering::Relaxed);
    }

    pub fn incr_heartbeats(&self) {
        self.heartbeats_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dump_json(&self) -> serde_json::Value {
        serde_json::json!({
            "refresh_count": self.refresh_count.load(Ordering::Relaxed),
            "rebuild_count": self.rebuild_count.load(Ordering::Relaxed),
            "bytes_sent": self.bytes_sent.load(Ordering::Relaxed),
            "heartbeats_sent": self.heartbeats_sent.load(Ordering::Relaxed),
        })
    }
}

lazy_static! {
    pub static ref TELEMETRY: Telemetry = Telemetry::new();
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub mac: String,
    pub ipv4: String,
    pub ipv6: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemInfo {
    // internal System from `sysinfo` — skip when serializing
    #[serde(skip)]
    sys: System,

    // previous network counters used to compute throughput
    #[serde(skip)]
    prev_network: HashMap<String, u64>,

    // last time this snapshot was taken (monotonic)
    #[serde(skip)]
    last_snapshot: Instant,

    // how frequently a snapshot is considered stale by default
    #[serde(skip)]
    pub snapshot_ttl: Duration,

    pub hostname: String,
    pub ip_address: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub uptime: String,

    pub cpu_brand: String,
    pub cpu_count: u32,
    pub cpu_usage: f32,

    pub ram_total: u64,
    pub ram_used: u64,
    pub ram_free: u64,

    pub disk_total: u64,
    pub disk_free: u64,
    pub disk_health: String,

    pub network_throughput: u64,
    pub network_interfaces: Vec<NetworkInterface>,

    pub architecture: String,

    pub device_type: String,
    pub device_model: String,
    pub serial_number: String,
}

impl SystemInfo {
    /// Synchronous construction (full snapshot). Use the async wrappers for non-blocking code.
    pub fn new() -> Self {
        // Build a System instance with expensive refresh kinds pre-selected
        let mut sys = System::new_with_specifics(
            RefreshKind::everything()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        sys.refresh_all();

        let (device_type, device_model, serial_number) = get_hardware_info();

        let hostname = System::host_name().unwrap_or_default();
        let os_name = System::name().unwrap_or_default();
        let os_version = System::os_version().unwrap_or_default();
        let kernel_version = System::kernel_version().unwrap_or_default();
        let uptime = format!("{}s", System::uptime());

        let ip_address = local_ip().ok().map(|ip: IpAddr| ip.to_string()).unwrap_or_default();

        let cpu_brand = sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_default();
        let cpu_count = sys.cpus().len() as u32;
        let cpu_usage = sys.global_cpu_usage();

        // Memory
        let ram_total = sys.total_memory();
        let ram_used = sys.used_memory();
        let ram_free = ram_total.saturating_sub(ram_used);

        // Disks
        let disks = Disks::new_with_refreshed_list_specifics(DiskRefreshKind::everything());
        let disk_total: u64 = disks.list().iter().map(|d| d.total_space()).sum();
        let disk_free: u64 = disks.list().iter().map(|d| d.available_space()).sum();

        let mut s = SystemInfo {
            sys,
            prev_network: HashMap::new(),
            last_snapshot: Instant::now(),
            snapshot_ttl: Duration::from_secs(5), // default TTL: 5s
            hostname,
            ip_address,
            os_name,
            os_version,
            kernel_version,
            uptime,
            cpu_brand,
            cpu_count,
            cpu_usage,
            ram_total,
            ram_used,
            ram_free,
            disk_total,
            disk_free,
            disk_health: "unknown".into(),
            network_throughput: 0,
            network_interfaces: Vec::new(),
            architecture: std::env::consts::ARCH.to_string(),
            device_type: device_type.unwrap_or_default(),
            device_model: device_model.unwrap_or_default(),
            serial_number: serial_number.unwrap_or_default(),
        };

        // populate network interfaces and throughput
        s.refresh_network_interfaces_blocking();
        TELEMETRY.incr_rebuild();
        s
    }

    /// Async non-blocking constructor (spawns a blocking task)
    pub async fn new_async() -> anyhow::Result<SystemInfo> {
        let si = tokio::task::spawn_blocking(|| SystemInfo::new())
            .await
            .context("Failed to build SystemInfo in blocking thread")?;
        Ok(si)
    }

    /// Rebuild a fresh snapshot (blocking) — useful if you want a full new snapshot
    pub fn rebuild_blocking(&self) -> SystemInfo {
        SystemInfo::new()
    }

    /// Rebuild a fresh snapshot (async)
    pub async fn rebuild_async() -> anyhow::Result<SystemInfo> {
        let si = tokio::task::spawn_blocking(|| SystemInfo::new())
            .await
            .context("Failed to rebuild SystemInfo in blocking thread")?;
        TELEMETRY.incr_rebuild();
        Ok(si)
    }

    /// Rebuild a snapshot only if `snapshot_ttl` has expired. Returns:
    /// - Ok(Some(new_snapshot)) if stale and rebuilt
    /// - Ok(None) if still fresh
    /// This is async-safe and non-blocking (uses spawn_blocking when rebuild is required).
    pub async fn rebuild_if_stale_async(&self) -> anyhow::Result<Option<SystemInfo>> {
        let elapsed = self.last_snapshot.elapsed();
        if elapsed < self.snapshot_ttl {
            return Ok(None);
        }

        // Rebuild in blocking thread
        let si = tokio::task::spawn_blocking(|| SystemInfo::new())
            .await
            .context("Failed to rebuild SystemInfo in blocking thread")?;
        TELEMETRY.incr_rebuild();
        Ok(Some(si))
    }

    /// Blocking refresh of the held SystemInfo instance (updates fields in-place).
    /// This is the original refresh behavior; use carefully (call from spawn_blocking in async context).
    pub fn refresh_blocking(&mut self) {
        self.sys.refresh_specifics(
            RefreshKind::everything()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );

        self.hostname = System::host_name().unwrap_or_default();
        self.os_name = System::name().unwrap_or_default();
        self.os_version = System::os_version().unwrap_or_default();
        self.kernel_version = System::kernel_version().unwrap_or_default();
        self.uptime = format!("{}s", System::uptime());

        self.ip_address = local_ip().ok().map(|ip| ip.to_string()).unwrap_or_default();

        // CPU
        self.cpu_usage = self.sys.global_cpu_usage();

        // Memory
        self.ram_total = self.sys.total_memory();
        self.ram_used = self.sys.used_memory();
        self.ram_free = self.ram_total.saturating_sub(self.ram_used);

        // Disks
        let disks = Disks::new_with_refreshed_list_specifics(DiskRefreshKind::everything());
        self.disk_total = disks.list().iter().map(|d| d.total_space()).sum();
        self.disk_free = disks.list().iter().map(|d| d.available_space()).sum();

        // Networks & interfaces (blocking)
        self.refresh_network_interfaces_blocking();

        self.last_snapshot = Instant::now();
        TELEMETRY.incr_refresh();
    }

    /// Async wrapper that refreshes in a blocking thread if TTL expired. Returns true if refreshed.
    pub async fn refresh_throttled_async(&mut self) -> anyhow::Result<bool> {
        let elapsed = self.last_snapshot.elapsed();
        if elapsed < self.snapshot_ttl {
            return Ok(false);
        }

        // Move necessary parts into a blocking task by replacing self with a placeholder.
        // We create a minimal mutable ownership to call `refresh_blocking`.
        // NOTE: this is safe because we ensure no other references exist while awaiting.
        // Implementation: perform a blocking refresh that mutates a new SystemInfo and then swap fields.
        let snapshot_ttl = self.snapshot_ttl;
        let current = std::mem::take(self);
        let refreshed = tokio::task::spawn_blocking(move || {
            let mut s = current;
            s.refresh_blocking();
            s
        })
        .await
        .context("Failed to refresh SystemInfo in blocking thread")?;

        // Put refreshed back into `self`
        *self = refreshed;
        self.snapshot_ttl = snapshot_ttl; // preserve requested TTL
        Ok(true)
    }

    /// Internal helper: blocking network interface refresh + throughput compute
    fn refresh_network_interfaces_blocking(&mut self) {
        let networks = Networks::new_with_refreshed_list();

        let mut all_ifaces: Vec<(String, NetworkInterface)> = Vec::new();

        for (name, iface) in networks.list().iter() {
            // try to read MAC (sysinfo's mac_address() uses platform internals)
            let mac = iface.mac_address().to_string();

            // sysinfo provides ip_networks with addr/ip/netmask (if supported)
            let ipv4 = iface
                .ip_networks()
                .iter()
                .filter_map(|n| if n.addr.is_ipv4() { Some(n.addr.to_string()) } else { None })
                .next()
                .unwrap_or_default();

            let ipv6 = iface
                .ip_networks()
                .iter()
                .filter_map(|n| if n.addr.is_ipv6() { Some(n.addr.to_string()) } else { None })
                .next()
                .unwrap_or_default();

            all_ifaces.push((
                name.clone(),
                NetworkInterface {
                    name: name.clone(),
                    mac,
                    ipv4,
                    ipv6,
                },
            ));
        }

        // Filter logic: Option 2 (prefer real physical NICs) but keep VM NICs when they have MAC/IP.
        let filtered: Vec<NetworkInterface> = all_ifaces
            .iter()
            .filter(|(name, ni)| {
                let name = name.as_str();

                // Drop loopback explicitly
                if name == "lo" || name == "lo0" {
                    return false;
                }

                // Drop obviously ephemeral container virtuals / bridges / veth / cni / kube etc.
                let virtual_prefixes = [
                    "docker", "br-", "veth", "cni", "kube", "vbox", "virbr", "vmnet", "vnet", "veth",
                ];
                if virtual_prefixes.iter().any(|p| name.starts_with(p)) {
                    return false;
                }

                // Drop VPN device prefixes that are usually not the primary interface (unless they have public IPs)
                let vpn_prefixes = ["tun", "tap", "wg", "zt", "wg-"];
                if vpn_prefixes.iter().any(|p| name.starts_with(p)) {
                    // keep if it has a routable IP (heuristic: non-empty ipv4 and not private? we'll just check non-empty)
                    if ni.ipv4.is_empty() && ni.ipv6.is_empty() {
                        return false;
                    }
                }

                // Keep if has MAC or IP — ensures VM NICs (which are virtual) are preserved when useful
                if ni.mac.is_empty() && ni.ipv4.is_empty() && ni.ipv6.is_empty() {
                    return false;
                }

                true
            })
            .map(|(_, iface)| iface.clone())
            .collect();

        // If we filtered everything out (e.g., single virtual machine with uncommon names), fall back to best-effort list:
        self.network_interfaces = if filtered.is_empty() {
            all_ifaces.into_iter().map(|(_, iface)| iface).collect()
        } else {
            filtered
        };

        // Update throughput counters based on current network stats
        let mut total_delta: u64 = 0;
        for (name, iface) in networks.list().iter() {
            let current = iface.received() + iface.transmitted();
            let prev = *self.prev_network.get(name).unwrap_or(&current);
            total_delta += current.saturating_sub(prev);
            self.prev_network.insert(name.clone(), current);
        }
        self.network_throughput = total_delta;
    }

    /// Returns a light-weight CPU usage value without doing full refresh.
    /// This method calls sys.refresh_cpu() only.
    pub fn cpu_usage_light(&mut self) -> f32 {
        self.sys.refresh_cpu();
        self.sys.global_cpu_usage()
    }

    /// Returns disk usage computed from sysinfo without performing a full snapshot.
    pub fn disk_usage_light(&self) -> (u64, u64) {
        let disks = Disks::new_with_refreshed_list_specifics(DiskRefreshKind::everything());
        let total = disks.list().iter().map(|d| d.total_space()).sum();
        let free = disks.list().iter().map(|d| d.available_space()).sum();
        (total, free)
    }

    /// Return telemetry metrics JSON for this snapshot (merges global telemetry)
    pub fn metrics_snapshot(&self) -> serde_json::Value {
        let mut map = serde_json::json!({
            "hostname": self.hostname,
            "uptime": self.uptime,
            "cpu_usage": self.cpu_usage,
            "ram_total": self.ram_total,
            "ram_used": self.ram_used,
            "disk_total": self.disk_total,
            "disk_free": self.disk_free,
            "network_throughput": self.network_throughput,
            "interfaces_count": self.network_interfaces.len(),
        });

        let telemetry = TELEMETRY.dump_json();
        if let serde_json::Value::Object(mut m) = map {
            if let serde_json::Value::Object(t) = telemetry {
                m.insert("telemetry".to_string(), serde_json::Value::Object(t));
            }
            serde_json::Value::Object(m)
        } else {
            map
        }
    }
}

impl Default for SystemInfo {
    fn default() -> Self {
        SystemInfo::new()
    }
}

/// Async helper to fetch a fresh snapshot (convenience wrapper)
pub async fn get_system_info_async() -> anyhow::Result<SystemInfo> {
    SystemInfo::new_async().await
}

/// get_system_info kept for API compatibility
pub fn get_system_info() -> anyhow::Result<SystemInfo> {
    Ok(SystemInfo::new())
}

/// Windows PCSystemTypeEx decoder (heuristic): takes a numeric string (WMI-field) and maps common values.
/// If parsing fails or code unknown, returns the original string or "unknown(<code>)".
fn decode_pc_system_type(raw: &str) -> String {
    match raw.parse::<i32>() {
        Ok(1) => "Desktop".into(),
        Ok(2) => "Mobile".into(),
        Ok(3) => "Workstation".into(),
        Ok(4) => "Enterprise Server".into(),
        Ok(5) => "SOHO Server".into(),
        Ok(6) => "Appliance PC".into(),
        Ok(7) => "Performance Server".into(),
        Ok(8) => "Maximum".into(),
        Ok(n) => format!("unknown({})", n),
        Err(_) => raw.to_string(),
    }
}

fn get_hardware_info() -> (Option<String>, Option<String>, Option<String>) {
    #[cfg(target_os = "linux")]
    {
        let dmi_path = "/sys/devices/virtual/dmi/id/";
        let read = |name: &str| -> Option<String> {
            fs::read_to_string(format!("{dmi_path}{name}"))
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };
        return (read("product_family"), read("product_name"), read("product_serial"));
    }

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Com::*;
        use windows::Win32::System::Wmi::*;
        use windows::core::*;

        unsafe {
            CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED).ok();

            let locator: IWbemLocator =
                CoCreateInstance(&CLSID_WbemLocator, None, CLSCTX_INPROC_SERVER).unwrap();

            let services = locator
                .ConnectServer(
                    &BSTR::from("ROOT\\CIMV2"),
                    &BSTR::new(),
                    &BSTR::new(),
                    &BSTR::new(),
                    0,
                    &BSTR::new(),
                    None,
                )
                .unwrap();

            let mut enumerator = None;
            services
                .ExecQuery(
                    &BSTR::from("WQL"),
                    &BSTR::from("SELECT * FROM Win32_ComputerSystem"),
                    WBEM_FLAG_FORWARD_ONLY,
                    None,
                    &mut enumerator,
                )
                .ok();

            let enumerator = match enumerator {
                Some(e) => e,
                None => return (None, None, None),
            };

            let mut device_type_raw: Option<String> = None;
            let mut device_model: Option<String> = None;

            loop {
                let mut obj = None;
                if enumerator.Next(WBEM_INFINITE, 1, &mut obj, std::ptr::null_mut()) != 0 {
                    break;
                }
                if let Some(obj) = obj {
                    // PCSystemTypeEx is typically numeric; decode to a friendly string
                    if let Some(raw) = get_wmi_string(&obj, "PCSystemTypeEx") {
                        device_type_raw = Some(decode_pc_system_type(&raw));
                    }
                    if device_model.is_none() {
                        device_model = get_wmi_string(&obj, "Model");
                    }
                }
            }

            let mut bios_enum = None;
            services
                .ExecQuery(
                    &BSTR::from("WQL"),
                    &BSTR::from("SELECT SerialNumber FROM Win32_BIOS"),
                    WBEM_FLAG_FORWARD_ONLY,
                    None,
                    &mut bios_enum,
                )
                .ok();

            let mut serial_number = None;
            if let Some(bios_enum) = bios_enum {
                loop {
                    let mut obj = None;
                    if bios_enum.Next(WBEM_INFINITE, 1, &mut obj, std::ptr::null_mut()) != 0 {
                        break;
                    }
                    if let Some(obj) = obj {
                        serial_number = get_wmi_string(&obj, "SerialNumber");
                    }
                }
            }

            return (device_type_raw, device_model, serial_number);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("system_profiler")
            .arg("-json")
            .arg("SPHardwareDataType")
            .output()
            .ok()?;
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        let hw = json.get("SPHardwareDataType")?.get(0)?;

        return (
            hw.get("machine_model").and_then(|v| v.as_str()).map(str::to_string),
            hw.get("model_name").and_then(|v| v.as_str()).map(str::to_string),
            hw.get("serial_number").and_then(|v| v.as_str()).map(str::to_string),
        );
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        (Some("unknown".into()), Some("unknown".into()), Some("unknown".into()))
    }
}

#[cfg(target_os = "windows")]
unsafe fn get_wmi_string(obj: &IWbemClassObject, field: &str) -> Option<String> {
    use windows::Win32::System::Variant::*;

    let mut vt_prop = VARIANT::default();
    if obj
        .Get(
            &BSTR::from(field),
            0,
            &mut vt_prop,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
        .is_err()
    {
        return None;
    }

    if vt_prop.Anonymous.Anonymous.vt as u32 != VT_BSTR.0 {
        return None;
    }

    Some(vt_prop.Anonymous.Anonymous.Anonymous.bstrVal.to_string())
}

// convenience import for anyhow::Context (used in async wrappers)
trait ContextCompat<T> {
    fn context(self, ctx: &'static str) -> anyhow::Result<T>;
}
impl<T> ContextCompat<T> for Result<T, tokio::task::JoinError> {
    fn context(self, ctx: &'static str) -> anyhow::Result<T> {
        self.map_err(|e| anyhow::anyhow!("{}: {}", ctx, e))
    }
}
