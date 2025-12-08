use std::collections::HashMap;
use std::fs;
use std::net::IpAddr;

#[cfg(target_os = "macos")]
use std::process::Command;

use local_ip_address::local_ip;
use serde::{Serialize, Deserialize};
use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, RefreshKind, System,
    Networks, Disks, DiskRefreshKind
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub mac: String,
    pub ipv4: String,
    pub ipv6: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemInfo {
    #[serde(skip)]
    sys: System,

    #[serde(skip)]
    prev_network: HashMap<String, u64>,

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
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::everything()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
        );
        sys.refresh_all();

        let (device_type, device_model, serial_number) = get_hardware_info();

        let hostname = System::host_name().unwrap_or_default();
        let os_name = System::name().unwrap_or_default();
        let os_version = System::os_version().unwrap_or_default();
        let kernel_version = System::kernel_version().unwrap_or_default();
        let uptime = format!("{}s", System::uptime());

        let ip_address = local_ip().ok().map(|ip: IpAddr| ip.to_string()).unwrap_or_default();

        // CPU
        let cpu_brand = sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_default();
        let cpu_count = sys.cpus().len() as u32;
        let cpu_usage = if sys.cpus().is_empty() { 0.0 } else { sys.global_cpu_usage() };

        // Memory
        let ram_total = sys.total_memory();
        let ram_used = sys.used_memory();
        let ram_free = ram_total.saturating_sub(ram_used);

        // Disks
        let disks = Disks::new_with_refreshed_list_specifics(DiskRefreshKind::everything());
        let disk_total: u64 = disks.list().iter().map(|d| d.total_space()).sum();
        let disk_free: u64 = disks.list().iter().map(|d| d.available_space()).sum();

        // Networks (name only for stability)
        let networks_raw = Networks::new_with_refreshed_list();
        let network_interfaces: Vec<NetworkInterface> = networks_raw
            .list()
            .iter()
            .map(|(name, _iface)| NetworkInterface {
                name: name.clone(),
                mac: String::new(),
                ipv4: String::new(),
                ipv6: String::new(),
            })
            .collect();

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
            disk_health: "unknown".into(),

            network_throughput: 0,
            network_interfaces,

            architecture: std::env::consts::ARCH.to_string(),

            device_type: device_type.unwrap_or_default(),
            device_model: device_model.unwrap_or_default(),
            serial_number: serial_number.unwrap_or_default(),
        }
    }

    /// Refresh all system stats.
    pub fn refresh(&mut self) {
        self.sys.refresh_specifics(
            RefreshKind::everything()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
        );

        // Host info
        self.hostname = System::host_name().unwrap_or_default();
        self.os_name = System::name().unwrap_or_default();
        self.os_version = System::os_version().unwrap_or_default();
        self.kernel_version = System::kernel_version().unwrap_or_default();
        self.uptime = format!("{}s", System::uptime());

        // IP might change
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

        // Networks: stable minimal interface list
        let networks = Networks::new_with_refreshed_list();
        self.network_interfaces = networks
            .list()
            .iter()
            .map(|(name, _)| NetworkInterface {
                name: name.clone(),
                mac: String::new(),
                ipv4: String::new(),
                ipv6: String::new(),
            })
            .collect();
    }

    /// Current CPU usage without using removed API.
    pub fn cpu_usage(&mut self) -> f32 {
        self.refresh();
        self.cpu_usage
    }

    /// Disk usage — compatible with sysinfo 0.37.x.
    pub fn disk_usage(&mut self) -> (u64, u64) {
        self.refresh();
        (self.disk_total, self.disk_free)
    }

    /// Network throughput (bytes since last call)
    pub fn network_throughput(&mut self) -> u64 {
        let networks = Networks::new_with_refreshed_list();
        let mut total = 0;

        for (name, iface) in networks.list().iter() {
            let current = iface.received() + iface.transmitted();
            let prev = *self.prev_network.get(name).unwrap_or(&current);
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
                if enumerator.Next(WBEM_INFINITE, 1, &mut obj, std::ptr::null_mut()) != 0 { break; }
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
                if bios_enum.Next(WBEM_INFINITE, 1, &mut obj, std::ptr::null_mut()) != 0 { break; }
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
    if obj.Get(
        &BSTR::from(field),
        0,
        &mut vt_prop,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    ).is_err() {
        return None;
    }

    if vt_prop.Anonymous.Anonymous.vt as u32 != VT_BSTR.0 {
        return None;
    }

    Some(vt_prop.Anonymous.Anonymous.Anonymous.bstrVal.to_string())
}
