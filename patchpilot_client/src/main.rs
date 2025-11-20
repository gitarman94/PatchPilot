mod system_info;
mod service;

use system_info::SystemInfo;
use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};
use std::error::Error;

fn setup_logger() -> Result<(), Box<dyn Error>> {
    Logger::try_with_str("info")?
        .log_to_file(FileSpec::default().directory("logs"))
        .rotate(
            Criterion::Age(flexi_logger::Age::Day),
            Naming::Numbers,
            Cleanup::KeepLogFiles(7),
        )
        .duplicate_to_stdout(Duplicate::All)
        .start()?;
    Ok(())
}

/// Log initial system snapshot at service startup.
/// This is useful for debugging and verifying hardware values.
fn log_initial_system_info() {
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
        info.ram_total,
        info.ram_used,
        info.ram_free
    );
    log::info!("Disk: total {} bytes, free {} bytes", disk_total, disk_free);
    log::info!("Network throughput (initial): {} bytes", net);
    log::info!("IP Address: {:?}", info.ip_address);
    log::info!("Architecture: {}", info.architecture);
    log::info!("Device Type: {:?}", info.device_type);
    log::info!("Device Model: {:?}", info.device_model);
    log::info!("Serial Number: {:?}", info.serial_number);
}

fn main() -> Result<(), Box<dyn Error>> {
    setup_logger()?;
    log::info!("Starting PatchPilot client...");

    // Log system info once at startup
    log_initial_system_info();

    // Start service loop (OS-specific)
    #[cfg(unix)]
    service::run_unix_service()?;

    #[cfg(windows)]
    service::run_service()?;

    Ok(())
}
