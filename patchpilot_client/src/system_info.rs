use std::collections::HashMap;
use std::fs;
use std::net::IpAddr;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use std::process::Command;

use local_ip_address::local_ip;
use serde::{Deserialize, Serialize};
use sysinfo::{
    CpuRefreshKind, DiskRefreshKind, Disks, MemoryRefreshKind, Networks, RefreshKind, System,
};

// Network interface representation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub mac: String,
    pub ipv4: String,
    pub ipv6: String,
}

// Public system snapshot structure (serializable).
#[derive(Serialize, Debug)]
pub struct SystemInfo {
    // sysinfo System kept private (not serialized)
    #[serde(skip)]
    sys: System,

    // previous network counters used for lightweight throughput calculations (not serialized)
    #[serde(skip)]
    prev_network: HashMap<String, u64>,

    // exposed fields
    pub hostname: String,
    pub ip_address: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub uptime: String,

    pub cpu_brand: String,
    pub cpu_count: u32,
    // current CPU usage percent
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

    // Optional ping latency (ms) placeholder; populate externally if desired.
    pub ping_latency: f32,
}

impl SystemInfo {
    // Blocking gather that performs an expensive, full snapshot.
    // Intended to be called inside spawn_blocking for async contexts.
    pub fn gather_blocking() -> SystemInfo {
        // Build sysinfo::System with detailed refresh kinds
        let mut sys = System::new_with_specifics(
            RefreshKind::everything()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        // full sync update
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

        // Disks (fresh list)
        let disks = Disks::new_with_refreshed_list_specifics(DiskRefreshKind::everything());
        let disk_total: u64 = disks.list().iter().map(|d| d.total_space()).sum();
        let disk_free: u64 = disks.list().iter().map(|d| d.available_space()).sum();

        let mut s = SystemInfo {
            sys,
            prev_network: HashMap::new(),
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
            ping_latency: 0.0,
        };

        // gather network details and throughput
        s.refresh_network_interfaces_blocking();

        s
    }

    // Light-weight CPU usage refresh that only refreshes CPU data.
    // This is a blocking call on the calling thread; callers in async contexts should call this inside spawn_blocking or use the service.
    pub fn cpu_usage_light(&mut self) -> f32 {
        // sysinfo uses refresh_cpu_all in newer versions
        self.sys.refresh_cpu_all();
        let usage = self.sys.global_cpu_usage();
        self.cpu_usage = usage;
        usage
    }

    // Light-weight disk usage computed without full System reinitialization.
    pub fn disk_usage_light(&self) -> (u64, u64) {
        let disks = Disks::new_with_refreshed_list_specifics(DiskRefreshKind::everything());
        let total = disks.list().iter().map(|d| d.total_space()).sum();
        let free = disks.list().iter().map(|d| d.available_space()).sum();
        (total, free)
    }

    // Light-weight network throughput recompute (non-persistent).
    // This will update the internal prev_network counters so repeated calls can compute deltas.
    pub fn network_throughput_light(&mut self) -> u64 {
        let networks = Networks::new_with_refreshed_list();
        let mut total: u64 = 0;
        for (name, iface) in networks.list().iter() {
            let current = iface.received() + iface.transmitted();
            let prev = *self.prev_network.get(name).unwrap_or(&current);
            total += current.saturating_sub(prev);
            self.prev_network.insert(name.clone(), current);
        }
        self.network_throughput = total;
        total
    }

    // Blocking network interfaces refresh + throughput compute (used by full gather).
    fn refresh_network_interfaces_blocking(&mut self) {
        let networks = Networks::new_with_refreshed_list();

        let mut all_ifaces: Vec<(String, NetworkInterface)> = Vec::new();

        for (name, iface) in networks.list().iter() {
            // read MAC / ip networks
            let mac = iface.mac_address().to_string();

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

        // Filter: prefer physical but keep VM NICs with MAC/IP
        let filtered: Vec<NetworkInterface> = all_ifaces
            .iter()
            .filter(|(name, ni)| {
                let name = name.as_str();

                // Drop loopback
                if name == "lo" || name == "lo0" {
                    return false;
                }

                // drop obvious container/bridge/veth prefixes
                let virtual_prefixes = [
                    "docker", "br-", "veth", "cni", "kube", "vbox", "virbr", "vmnet", "vnet",
                ];
                if virtual_prefixes.iter().any(|p| name.starts_with(p)) {
                    return false;
                }

                // vpn prefixes: keep only if they have IPs
                let vpn_prefixes = ["tun", "tap", "wg", "zt", "wg-"];
                if vpn_prefixes.iter().any(|p| name.starts_with(p)) {
                    if ni.ipv4.is_empty() && ni.ipv6.is_empty() {
                        return false;
                    }
                }

                // Must have either MAC or an IP to be useful
                if ni.mac.is_empty() && ni.ipv4.is_empty() && ni.ipv6.is_empty() {
                    return false;
                }

                true
            })
            .map(|(_, iface)| iface.clone())
            .collect();

        self.network_interfaces = if filtered.is_empty() {
            all_ifaces.into_iter().map(|(_, iface)| iface).collect()
        } else {
            filtered
        };

        // throughput counters (delta since previous)
        let mut total_delta: u64 = 0;
        for (name, iface) in networks.list().iter() {
            let current = iface.received() + iface.transmitted();
            let prev = *self.prev_network.get(name).unwrap_or(&current);
            total_delta += current.saturating_sub(prev);
            self.prev_network.insert(name.clone(), current);
        }
        self.network_throughput = total_delta;
    }
}

// Synchronous compatibility helper — blocking gather.
pub fn get_system_info() -> anyhow::Result<SystemInfo> {
    Ok(SystemInfo::gather_blocking())
}

// Service that enforces a lightweight async rate-limit while always returning fresh data.
// Create one and re-use it across the program to avoid per-call throttling races.
pub struct SystemInfoService {
    last_call: tokio::sync::Mutex<Instant>,
    min_interval: Duration,
}

impl SystemInfoService {
    // Create a new service with a user-specified minimum interval between actual gathers.
    // The interval is a *rate-limit* — if calls happen more quickly, they'll asynchronously wait until the interval expires.
    pub fn new(min_interval: Duration) -> Self {
        // initialize last_call to a time far in the past so first call is immediate
        let last = Instant::now() - min_interval - Duration::from_millis(1);
        Self {
            last_call: tokio::sync::Mutex::new(last),
            min_interval,
        }
    }

    // Default service: 200ms minimum interval (lightweight).
    pub fn default() -> Self {
        Self::new(Duration::from_millis(200))
    }

    // Async method to fetch a fresh SystemInfo snapshot while enforcing the lightweight rate-limit.
    // This method:
    // - asynchronously waits if the previous gather was within `min_interval` (no blocking threads),
    // - spawns a blocking task to run the expensive `gather_blocking` and returns the fresh snapshot.
    pub async fn get_system_info_async(&self) -> anyhow::Result<SystemInfo> {
        // determine how long to wait (if anything) in an async-safe way
        let mut guard = self.last_call.lock().await;
        let now = Instant::now();
        if let Some(remaining) = self
            .min_interval
            .checked_sub(now.duration_since(*guard))
        {
            if remaining > Duration::from_millis(0) {
                // asynchronous sleep; does not block the runtime thread
                tokio::time::sleep(remaining).await;
            }
        }
        // update last_call to now (we're about to perform the gather)
        *guard = Instant::now();
        drop(guard);

        // perform blocking gather in threadpool
        let si = tokio::task::spawn_blocking(|| SystemInfo::gather_blocking())
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?;

        Ok(si)
    }
}

// Convenience async helper that uses the default service instance (200ms min interval).
pub async fn get_system_info_async_default() -> anyhow::Result<SystemInfo> {
    let svc = SystemInfoService::default();
    svc.get_system_info_async().await
}

// Platform helpers
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
            .ok()?; // return None on failure
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

// Heuristic Windows PCSystemTypeEx decoder (only compiled on Windows).
#[cfg(target_os = "windows")]
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
