use std::collections::HashMap;
use std::fs;
use std::net::IpAddr;
use std::process::Command;

use local_ip_address::local_ip;
use serde::Serialize;
use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, RefreshKind, System,
    Disks, DiskRefreshKind,
    Networks, NetworkData,
};

#[derive(Serialize, Default)]
pub struct SystemInfo {
    #[serde(skip)]
    sys: System,
    #[serde(skip)]
    prev_network: HashMap<String, u64>,

    pub hostname: Option<String>,
    pub ip_address: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub uptime: Option<String>,

    pub cpu_brand: Option<String>,
    pub cpu_count: Option<usize>,
    pub cpu_usage: Option<f32>,

    pub ram_total: u64,
    pub ram_used: u64,
    pub ram_free: u64,

    pub disk_total: u64,
    pub disk_free: u64,
    pub disk_health: Option<String>,

    pub network_throughput: u64,
    pub network_interfaces: Option<String>,

    pub architecture: String,

    pub device_type: Option<String>,
    pub device_model: Option<String>,
    pub serial_number: Option<String>,
}

impl SystemInfo {
    pub fn new() -> Self {
        // Create system instance with full refresh settings
        let mut sys = System::new_with_specifics(
            RefreshKind::everything()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
        );
        sys.refresh_all();

        // Hardware info
        let (device_type, device_model, serial_number) = get_hardware_info();

        // Static-ish system info
        let hostname = System::host_name();
        let os_name = System::name();
        let os_version = System::os_version();
        let kernel_version = System::kernel_version();
        let uptime = Some(format!("{}s", System::uptime()));

        // IP address
        let ip_address = local_ip().ok().map(|ip: IpAddr| ip.to_string());

        // CPU info
        let cpu_brand = sys.cpus().first().map(|c| c.brand().to_string());
        let cpu_count = Some(sys.cpus().len());
        let cpu_usage = if !sys.cpus().is_empty() {
            Some(sys.global_cpu_usage())
        } else {
            None
        };

        // Memory info
        let ram_total = sys.total_memory();
        let ram_used = sys.used_memory();
        let ram_free = ram_total.saturating_sub(ram_used);

        // Disks: use new Disks API
        let mut disks = Disks::new_with_refreshed_list_specifics(DiskRefreshKind::everything());
        let disk_total: u64 = disks.list().iter().map(|d| d.total_space()).sum();
        let disk_free: u64 = disks.list().iter().map(|d| d.available_space()).sum();

        // Networks: use new Networks API
        let mut networks = Networks::new_with_refreshed_list();
        // `refresh(false)` so we don't remove interfaces right away
        networks.refresh(false);
        let network_interfaces = if networks.list().is_empty() {
            None
        } else {
            Some(
                networks
                    .list()
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        };

        SystemInfo {
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
            disk_health: Some("unknown".to_string()),

            network_throughput: 0,
            network_interfaces,

            architecture: std::env::consts::ARCH.to_string(),

            device_type,
            device_model,
            serial_number,
        }
    }

    pub fn refresh(&mut self) {
        // Refresh system data
        self.sys
            .refresh_specifics(RefreshKind::everything());

        // Update hostname, os, uptime
        self.hostname = System::host_name();
        self.os_name = System::name();
        self.os_version = System::os_version();
        self.kernel_version = System::kernel_version();
        self.uptime = Some(format!("{}s", System::uptime()));

        // IP address might change
        self.ip_address = local_ip().ok().map(|ip| ip.to_string());

        // CPU usage
        self.cpu_usage = Some(self.sys.global_cpu_usage());

        // Memory
        self.ram_total = self.sys.total_memory();
        self.ram_used = self.sys.used_memory();
        self.ram_free = self.ram_total.saturating_sub(self.ram_used);

        // Disks: refresh via Disks struct
        let mut disks = Disks::new_with_refreshed_list_specifics(DiskRefreshKind::everything());
        let total = disks.list().iter().map(|d| d.total_space()).sum();
        let free = disks.list().iter().map(|d| d.available_space()).sum();
        self.disk_total = total;
        self.disk_free = free;

        // Networks: refresh and compute interfaces
        let mut networks = Networks::new_with_refreshed_list();
        networks.refresh(false);
        self.network_interfaces = if networks.list().is_empty() {
            None
        } else {
            Some(
                networks
                    .list()
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        };
    }

    pub fn cpu_usage(&mut self) -> f32 {
        self.sys.refresh_cpu_all();
        self.sys.global_cpu_usage()
    }

    pub fn disk_usage(&mut self) -> (u64, u64) {
        // Refresh everything so disk list and data is up to date
        self.sys.refresh_all();
        let mut disks = Disks::new_with_refreshed_list_specifics(DiskRefreshKind::everything());
        let total = disks.list().iter().map(|d| d.total_space()).sum();
        let free = disks.list().iter().map(|d| d.available_space()).sum();
        (total, free)
    }

    pub fn network_throughput(&mut self) -> u64 {
        // Use Networks struct to get data
        let mut networks = Networks::new_with_refreshed_list();
        // Remove old interfaces? here we choose false so as to keep keys
        networks.refresh(false);

        let mut total: u64 = 0;

        for (name, data) in networks.list().iter() {
            let current = data.received() + data.transmitted();
            let prev = self.prev_network.get(name).copied().unwrap_or(current);
            total += current.saturating_sub(prev);
            self.prev_network.insert(name.clone(), current);
        }

        self.network_throughput = total;
        total
    }
}

pub fn get_system_info() -> anyhow::Result<SystemInfo> {
    Ok(SystemInfo::new())
}

//
// HARDWARE SERIAL + MODEL + DEVICE TYPE (OS‑SPECIFIC)
//
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
        return (
            read("product_family"),
            read("product_name"),
            read("product_serial"),
        );
    }

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Wmi::*;
        use windows::Win32::System::Com::*;
        use windows::core::*;

        unsafe {
            CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED).ok();
            let locator: IWbemLocator = CoCreateInstance(
                &CLSID_WbemLocator,
                None,
                CLSCTX_INPROC_SERVER,
            ).unwrap();
            let services = locator.ConnectServer(
                &BSTR::from("ROOT\\CIMV2"),
                &BSTR::new(),
                &BSTR::new(),
                &BSTR::new(),
                0,
                &BSTR::new(),
                None,
            ).unwrap();

            let mut enumerator = None;
            services.ExecQuery(
                &BSTR::from("WQL"),
                &BSTR::from("SELECT * FROM Win32_ComputerSystem"),
                WBEM_FLAG_FORWARD_ONLY,
                None,
                &mut enumerator,
            ).ok();
            let enumerator = enumerator.unwrap();

            let mut device_type = None;
            let mut device_model = None;
            loop {
                let mut obj = None;
                let ret = enumerator.Next(WBEM_INFINITE, 1, &mut obj, std::ptr::null_mut());
                if ret != 0 { break; }
                if let Some(obj) = obj {
                    device_type = get_wmi_string(&obj, "PCSystemTypeEx");
                    device_model = get_wmi_string(&obj, "Model");
                }
            }

            let mut bios_enum = None;
            services.ExecQuery(
                &BSTR::from("WQL"),
                &BSTR::from("SELECT SerialNumber FROM Win32_BIOS"),
                WBEM_FLAG_FORWARD_ONLY,
                None,
                &mut bios_enum,
            ).ok();
            let bios_enum = bios_enum.unwrap();
            let mut serial_number = None;
            loop {
                let mut obj = None;
                let ret = bios_enum.Next(WBEM_INFINITE, 1, &mut obj, std::ptr::null_mut());
                if ret != 0 { break; }
                if let Some(obj) = obj {
                    serial_number = get_wmi_string(&obj, "SerialNumber");
                }
            }

            return (device_type, device_model, serial_number);
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
    let hr = obj.Get(
        &BSTR::from(field),
        0,
        &mut vt_prop,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    if hr.is_err() {
        return None;
    }
    if vt_prop.Anonymous.Anonymous.vt as u32 != VT_BSTR.0 {
        return None;
    }
    Some(vt_prop.Anonymous.Anonymous.Anonymous.bstrVal.to_string())
}
