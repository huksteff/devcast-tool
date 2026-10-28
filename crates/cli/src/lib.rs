//! devcast command-line tool: edit, analyse, render and export projects.

mod edit;
pub mod error;
pub mod find;
pub mod globals;
mod help;
mod help_args;
mod ls;
pub mod registry;
pub mod run;
pub mod workspace;

#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod ls_tests;
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
