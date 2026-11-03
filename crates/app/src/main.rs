//! DevCast desktop app: HUD, recording windows, editor, timeline, library, settings.

// No console window in release builds (decision 001).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    dc_update::handle_installer_hooks();
    match dc_app::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
