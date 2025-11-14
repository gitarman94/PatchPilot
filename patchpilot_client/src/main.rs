mod system_info;

use system_info::collect_system_info;
use flexi_logger::{Logger, FileSpec, Duplicate, Criterion, Naming, Cleanup};
use std::error::Error;

fn setup_logger() -> Result<(), Box<dyn Error>> {
    Logger::try_with_str("info")?
        .log_to_file(FileSpec::default().directory("logs"))
        .duplicate_to_stdout(Duplicate::All)
        .rotate(
            Criterion::Age(flexi_logger::Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(7),
        )
        .start()?;
    Ok(())
}

fn print_system_info(info: &system_info::SystemInfo) {
    println!("=== System Information ===");
    println!("Hostname: {}", info.hostname);
    println!("OS: {} ({})", info.os_name, info.architecture);
    println!("CPU Usage: {:.2}%", info.cpu);
    println!("RAM: total {} MB, used {} MB, free {} MB",
        info.ram_total / 1024,
        info.ram_used / 1024,
        info.ram_free / 1024
    );
    println!("Disk: total {} GB, free {} GB, health: {}",
        info.disk_total / (1024*1024*1024),
        info.disk_free / (1024*1024*1024),
        info.disk_health
    );
    println!("Network throughput: {} bytes", info.network_throughput);
    if let Some(ref ifaces) = info.network_interfaces {
        println!("Network interfaces: {}", ifaces);
    }
    if let Some(ref ip) = info.ip_address {
        println!("IP address: {}", ip);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    setup_logger()?;
    log::info!("Starting system info collection...");

    let sys_info = collect_system_info();
    log::info!("System info collected successfully.");

    print_system_info(&sys_info);

    Ok(())
}
