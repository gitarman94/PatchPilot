mod system_info;

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

fn print_system_info(info: &mut SystemInfo) {
    info.refresh(); // Refresh data before printing

    let (disk_total, disk_free) = info.disk_usage();

    println!("=== System Information ===");
    println!("Hostname: {:?}", info.hostname());
    println!("OS: {} {}", info.os_name(), info.os_version());
    println!("Architecture: {}", info.architecture());
    println!("CPU Usage: {:.2}%", info.cpu_usage());
    println!(
        "RAM: total {} KB, free {} KB",
        info.ram_total(),
        info.ram_free()
    );
    println!(
        "Disk: total {} B, free {} B",
        disk_total, disk_free
    );
    println!("Network throughput: {} B/s", info.network_throughput());
    println!("IP Address: {:?}", info.ip_address());
}

fn main() -> Result<(), Box<dyn Error>> {
    setup_logger()?;
    log::info!("Starting system info collection...");

    let mut sys_info = SystemInfo::new();
    log::info!("System info object created.");

    print_system_info(&mut sys_info);
    log::info!("System info printed successfully.");

    Ok(())
}
