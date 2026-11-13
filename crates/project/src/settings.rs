//! The app's settings folder, shared by the editor and `devcast`: the defaults file and the
//! last recording settings lie there. Windows: `%APPDATA%\DevCast`.

use std::path::PathBuf;

use crate::error::{ProjectError, Result};

/// File of the app defaults ([`crate::Defaults`]) in [`dir`].
pub const DEFAULTS_FILE: &str = "defaults.json";

/// Folder of the app's log files in [`dir`]: «Открыть папку логов», `devcast app feedback`.
pub const LOGS_DIR: &str = "logs";

/// Folder of the crash reports in [`LOGS_DIR`]: minidumps and panics of `devcast-app`, sent
/// along with the logs.
pub const CRASHES_DIR: &str = "crashes";

pub fn dir() -> Result<PathBuf> {
    std::env::var_os("APPDATA")
        .map(|appdata| PathBuf::from(appdata).join("DevCast"))
        .ok_or(ProjectError::NoSettingsDir)
}

pub fn logs_dir() -> Result<PathBuf> {
    Ok(dir()?.join(LOGS_DIR))
}

pub fn defaults_file() -> Result<PathBuf> {
    Ok(dir()?.join(DEFAULTS_FILE))
}
