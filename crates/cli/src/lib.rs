//! devcast command-line tool: edit, analyse, render and export projects.

mod autozoom;
mod edit;
pub mod error;
mod events;
pub mod find;
pub mod globals;
mod help;
mod help_args;
mod import;
mod ls;
mod record;
mod record_args;
mod record_control;
pub mod registry;
pub mod run;
mod silences;
mod skill;
mod sources;
mod telemetry;
mod transcript;
pub mod workspace;

#[cfg(test)]
mod autozoom_tests;
#[cfg(test)]
mod e2e_tests;
#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod events_tests;
#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod ls_tests;
#[cfg(test)]
mod record_tests;
#[cfg(test)]
mod recover_tests;
#[cfg(test)]
mod skill_tests;
#[cfg(test)]
mod test_util;
#[cfg(test)]
mod tests;

use std::ffi::OsString;

pub use error::CliError;
pub use run::{Outcome, run};

/// `devcast` as the binary runs it: arguments from the OS, the current folder, all ops.
pub fn main_outcome(args: Vec<OsString>) -> Outcome {
    let prepared = args
        .into_iter()
        .map(|arg| {
            arg.into_string().map_err(|arg| {
                CliError::Usage(format!(
                    "аргумент «{}» не читается как текст UTF-8: переименуй файл или папку",
                    arg.to_string_lossy()
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .and_then(|args| {
            let cwd = std::env::current_dir().map_err(|e| {
                CliError::Failed(format!(
                    "текущая папка недоступна ({e}): перейди в существующую папку"
                ))
            })?;
            Ok((args, cwd, registry::registry()?))
        });
    match prepared {
        Ok((args, cwd, registry)) => run(args, &cwd, &registry),
        Err(error) => Outcome::failure(&error, false),
    }
}
#[cfg(test)]
mod silences_tests;
#[cfg(test)]
mod transcript_tests;
