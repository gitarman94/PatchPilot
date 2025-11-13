mod system_info;

use system_info::{collect_system_info, SystemInfo};
use flexi_logger::{Logger, FileSpec, Duplicate};
use std::error::Error;

fn setup_logger() -> Result<(), Box<dyn Error>> {
    Logger::try_with_str("info")?
        .log_to_file(FileSpec::default().directory("logs"))
        .duplicate_to_stdout(Duplicate::All)
        .start()?;
    Ok(())
}

fn print_system_info(info: &SystemInfo) {
    println!("=== System Information ===");
    println!("Hostname: {}", info.hostname);
    println!("OS: {} {}", info.os_name, info.os_version);
    println!("Kernel: {}", info.kernel_version);
    println!("Uptime (seconds): {}", info.uptime_seconds);

    println!("\n--- CPUs ---");
    for (i, cpu) in info.cpus.iter().enumerate() {
        println!("CPU {}: {} ({} cores)", i, cpu.name, cpu.cores);
    }

    println!("\n--- Disks ---");
    for disk in &info.disks {
        println!(
            "{}: total {} bytes, available {} bytes",
            disk.name, disk.total_space, disk.available_space
        );
    }

    println!("\n--- Network Interfaces ---");
    for net in &info.network_interfaces {
        println!(
            "{}: received {} bytes, transmitted {} bytes",
            net.name, net.received, net.transmitted
        );
    }

    println!("\n--- Processes (top 10 by memory) ---");
    let mut processes = info.processes.clone();
    processes.sort_by(|a, b| b.memory.cmp(&a.memory));
    for process in processes.iter().take(10) {
        println!(
            "PID {}: {} ({} KB)",
            process.pid,
            process.name,
            process.memory
        );
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    setup_logger()?;
    log::info!("Starting system info collection...");

    // Collect system info
    let sys_info = collect_system_info();
    log::info!("System info collected successfully.");

    // Print to console
    print_system_info(&sys_info);

    Ok(())
}
