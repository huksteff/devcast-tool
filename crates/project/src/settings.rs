//! The app's settings folder, shared by the editor and `devcast`: the defaults file and the
//! last recording settings lie there. Windows: `%APPDATA%\DevCast`.

use std::path::PathBuf;

use crate::error::{ProjectError, Result};

/// File of the app defaults ([`crate::Defaults`]) in [`dir`].
pub const DEFAULTS_FILE: &str = "defaults.json";

pub fn dir() -> Result<PathBuf> {
    std::env::var_os("APPDATA")
        .map(|appdata| PathBuf::from(appdata).join("DevCast"))
        .ok_or(ProjectError::NoSettingsDir)
}

/// The app defaults file.
pub fn defaults_file() -> Result<PathBuf> {
    Ok(dir()?.join(DEFAULTS_FILE))
}
