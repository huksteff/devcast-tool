//! Log files of the app with daily rotation, and panics written to the log and as crash reports.

use std::path::{Path, PathBuf};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::EnvFilter;

use crate::error::AppError;

/// Days of logs kept; older files are deleted at rotation.
const KEEP_FILES: usize = 7;

/// Writes the log into `dir` until the returned guard is dropped (it flushes the rest), panics
/// also as reports into `crashes`. `RUST_LOG` picks the level, `info` by default.
pub fn init(dir: &Path, crashes: &Path) -> Result<WorkerGuard, AppError> {
    let error = |message: String| AppError::Log {
        path: dir.to_path_buf(),
        message,
    };
    let appender = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("devcast-app")
        .filename_suffix("log")
        .max_log_files(KEEP_FILES)
        .build(dir)
        .map_err(|e| error(e.to_string()))?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_writer(writer)
        .with_ansi(false)
        .try_init()
        .map_err(|e| error(e.to_string()))?;
    log_panics(crashes.to_path_buf());
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "DevCast запущен");
    Ok(guard)
}

/// A panic goes to the log and to a crash report with its backtrace, then to the default hook.
fn log_panics(crashes: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let text = format!("паника: {info}\n{backtrace}");
        tracing::error!("{text}");
        if let Err(error) = crate::crashes::write_panic(&crashes, &text) {
            tracing::error!("отчёт о панике не записан: {error}");
        }
        previous(info);
    }));
}
