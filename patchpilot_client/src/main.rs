mod system_info;
mod service;

use crate::service::init_logging;

fn log_initial_system_info() {
    use system_info::SystemInfo;

    let mut info = SystemInfo::new();
    info.refresh();

    let (disk_total, disk_free) = info.disk_usage();
    let net = info.network_throughput();

    log::info!("=== Initial System Information ===");
    log::info!("Hostname: {:?}", info.hostname);
    log::info!("OS Name: {:?}", info.os_name);
    log::info!("OS Version: {:?}", info.os_version);
    log::info!("Kernel Version: {:?}", info.kernel_version);
    log::info!("CPU Usage: {:.2}%", info.cpu_usage());
    log::info!(
        "RAM: total {} KB, used {} KB, free {} KB",
        info.ram_total, info.ram_used, info.ram_free
    );
    log::info!("Disk: total {} bytes, free {} bytes", disk_total, disk_free);
    log::info!("Network throughput (initial): {} bytes", net);
    log::info!("IP Address: {:?}", info.ip_address);
    log::info!("Architecture: {}", info.architecture);
    log::info!("Device Type: {:?}", info.device_type);
    log::info!("Device Model: {:?}", info.device_model);
    log::info!("Serial Number: {:?}", info.serial_number);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging — guaranteed to write to a real file now.
    if let Err(e) = init_logging() {
        eprintln!("❌ Failed to initialize logging: {e}");
        return Err(Box::new(e));
    }

    log::info!("📌 PatchPilot client starting up...");
    //Immediately log system info to that file
    log_initial_system_info();
    //Launch the OS-specific service

    #[cfg(unix)]
    {
        if let Err(e) = service::run_unix_service().await {
            log::error!("Unix service error: {e}");
            return Err(Box::new(e));
        }
    }

    #[cfg(windows)]
    {
        if let Err(e) = service::run_service().await {
            log::error!("Windows service error: {e}");
            return Err(Box::new(e));
        }
    }

    Ok(())
}
