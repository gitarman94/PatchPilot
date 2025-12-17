use anyhow::Result;
use reqwest::Client;
use crate::device::run_adoption_and_update_loop;
use std::path::PathBuf;

pub fn init_logging() -> anyhow::Result<flexi_logger::LoggerHandle> {
    use flexi_logger::{
        Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming,
    };

    let log_dir: PathBuf = crate::get_base_dir().into();
    let log_dir = log_dir.join("logs");

    std::fs::create_dir_all(&log_dir)?;

    let handle = Logger::try_with_str("info")?
        .log_to_file(
            FileSpec::default()
                .directory(log_dir)
                .basename("patchpilot_client")
                .suffix("log"),
        )
        .rotate(
            Criterion::Size(5_000_000), // 5 MB
            Naming::Numbers,
            Cleanup::KeepLogFiles(10),
        )
        .duplicate_to_stderr(Duplicate::Info)
        .start()?;

    Ok(handle)
}

/// Unix service entrypoint
#[cfg(any(unix, target_os = "macos"))]
pub async fn run_unix_service() -> Result<()> {
    let client = Client::new();
    let server_url = crate::system_info::read_server_url().await?;

    // Run adoption/update loop and capture the device_id
    let device_id = run_adoption_and_update_loop(&client, &server_url, None).await?;

    // Start command polling for this device
    crate::action::command_poll_loop(
        client.clone(),
        server_url.to_string(),
        device_id.clone(),
        None,
    ).await;

    Ok(())
}

/// Windows service entrypoint
#[cfg(windows)]
pub async fn run_service() -> Result<()> {
    use windows_service::{
        service::{ServiceControl, ServiceControlHandlerResult},
        service_control_handler,
    };
    let running_flag = Arc::new(AtomicBool::new(true));
    let running_flag_clone = running_flag.clone();

    fn service_main(flag: Arc<AtomicBool>) -> Result<()> {
        let client = Client::new();
        let server_url = futures::executor::block_on(crate::system_info::read_server_url())?;

        // Register & get device ID
        let device_id = futures::executor::block_on(
            run_adoption_and_update_loop(&client, &server_url, Some(flag.clone()))
        )?;

        // Start polling for commands
        futures::executor::block_on(
            crate::action::command_poll_loop(
                client.clone(),
                server_url.clone(),
                device_id.clone(),
                Some(flag),
            )
        );

        Ok(())
    }

    let flag_for_handler = running_flag.clone();
    let _status = service_control_handler::register("PatchPilot", move |control| {
        match control {
            ServiceControl::Stop => {
                flag_for_handler.store(false, std::sync::atomic::Ordering::SeqCst);
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    })?;

    service_main(running_flag_clone)
}
