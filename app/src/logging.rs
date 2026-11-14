//! Structured logging to a daily-rotated file in the app log dir (plus stderr
//! in debug builds). Never log secret values; core types redact themselves.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Keep a week of daily logs.
const MAX_LOG_FILES: usize = 7;

pub struct LogGuard(#[allow(dead_code)] WorkerGuard);

pub fn init(dir: &Path) -> Result<LogGuard, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("nightgrid")
        .filename_suffix("log")
        .max_log_files(MAX_LOG_FILES)
        .build(dir)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let filter = EnvFilter::try_from_env("NIGHTGRID_LOG")
        .unwrap_or_else(|_| EnvFilter::new(if cfg!(debug_assertions) { "debug" } else { "info" }));

    let file_layer = fmt::layer().with_writer(writer).with_ansi(false).with_target(true);
    let stderr_layer = cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr));

    tracing_subscriber::registry().with(filter).with(file_layer).with(stderr_layer).try_init()?;
    Ok(LogGuard(guard))
}
