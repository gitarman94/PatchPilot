mod system_info;

use system_info::{collect_system_info, SystemInfo};
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

fn print_system_info(info: &SystemInfo) {
    println!("=== System Information ===");
    println!("Hostname: {}", info.hostname);
    println!("OS: {} {}", info.os_name, info.os_version);
    println!("Architecture: {}", info.architecture);
    println!("CPU Usage: {:.2}%", info.cpu);
    println!("RAM: total {} MB, used {} MB, free {} MB", 
        info.ram_total / 1024, info.ram_used / 1024, info.ram_free / 1024);
    println!("Disk: total {} GB, free {} GB, health {}", 
        info.disk_total / 1_000_000_000, info.disk_free / 1_000_000_000, info.disk_health);
    println!("Network throughput: {} B/s", info.network_throughput);
    println!("IP Address: {:?}", info.ip_address);
    println!("Hostname: {}", info.hostname);
}

fn main() -> Result<(), Box<dyn Error>> {
    setup_logger()?;
    log::info!("Starting system info collection...");

    let sys_info = collect_system_info();
    log::info!("System info collected successfully.");

    print_system_info(&sys_info);

    Ok(())
}
