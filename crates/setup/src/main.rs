//! `DevCast-Setup.exe`: asks where to install DevCast and installs it (`dc_setup::run`).

#![windows_subsystem = "windows"]

use std::process::ExitCode;

fn main() -> ExitCode {
    match dc_setup::run() {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            dc_setup::tell(&error);
            ExitCode::FAILURE
        }
    }
}
