use anyhow::Result;
use std::path::PathBuf;
use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};

/// Initialize logging for the PatchPilot client
pub fn init_logging() -> Result<flexi_logger::LoggerHandle> {
    let log_dir: PathBuf = crate::get_base_dir().into();
    let log_dir = log_dir.join("logs");
    std::fs::create_dir_all(&log_dir)?;

    let handle = Logger::try_with_str("info")?
        .log_to_file(
            FileSpec::default()
                .directory(&log_dir)
                .basename("patchpilot_client")
                .suffix("log"),
        )
        .rotate(
            Criterion::Size(5_000_000),
            Naming::Numbers,
            Cleanup::KeepLogFiles(10),
        )
        .duplicate_to_stderr(Duplicate::Info)
        .start()?;

    Ok(handle)
}
