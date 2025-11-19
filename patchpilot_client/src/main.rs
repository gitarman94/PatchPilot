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

fn print_system_info(info: &mut SystemInfo) {
    info.refresh();
    let (disk_total, disk_free) = info.disk_usage();
    let net = info.network_throughput();

    println!("=== System Information ===");
    println!("Hostname: {:?}", info.hostname());
    println!("OS Name: {:?}", info.os_name());
    println!("OS Version: {:?}", info.os_version());
    println!("Kernel Version: {:?}", info.kernel_version());
    println!("CPU Usage: {:.2}%", info.cpu_usage());
    println!(
        "RAM: total {} KB, used {} KB, free {} KB",
        info.ram_total(),
        info.ram_used(),
        info.ram_free()
    );
    println!("Disk: total {} bytes, free {} bytes", disk_total, disk_free);
    println!("Network throughput (delta): {} bytes", net);
    println!("IP Address: {:?}", info.ip_address());
}

fn main() -> Result<(), Box<dyn Error>> {
    setup_logger()?;
    log::info!("Starting PatchPilot client...");

    // Run actual service loop (instead of exiting immediately)
    #[cfg(unix)]
    service::run_unix_service()?;

    #[cfg(windows)]
    service::run_service()?;

    Ok(())
}
