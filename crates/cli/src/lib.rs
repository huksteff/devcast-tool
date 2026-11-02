//! devcast command-line tool: edit, analyse, render and export projects.

mod audio;
mod autozoom;
mod batch;
mod builtin;
mod builtin_groups;
mod captions;
mod defaults;
mod edit;
pub mod error;
mod events;
mod export;
mod export_args;
mod export_job;
mod export_queue;
pub mod find;
mod font;
pub mod globals;
mod grid;
mod help;
mod help_args;
mod image;
mod import;
mod live;
mod ls;
mod record;
mod record_args;
mod record_control;
mod round;
pub mod run;
mod scene;
mod silences;
mod skill;
mod sources;
mod speech;
mod still;
mod style;
mod telemetry;
mod transcript;
pub mod worker;
pub mod workspace;

#[cfg(test)]
mod audio_tests;
#[cfg(test)]
mod autozoom_tests;
#[cfg(test)]
mod captions_tests;
#[cfg(test)]
mod cut_e2e_tests;
#[cfg(test)]
mod e2e_tests;
#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod events_tests;
#[cfg(test)]
mod export_tests;
#[cfg(test)]
mod font_tests;
#[cfg(test)]
mod image_tests;
#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod live_tests;
#[cfg(test)]
mod ls_tests;
#[cfg(test)]
mod pauses_e2e_tests;
#[cfg(test)]
mod record_tests;
#[cfg(test)]
mod recover_tests;
#[cfg(test)]
mod scope_e2e_frames;
#[cfg(test)]
mod scope_e2e_tests;
#[cfg(test)]
mod silences_tests;
#[cfg(test)]
mod skill_tests;
#[cfg(test)]
mod still_tests;
#[cfg(test)]
mod style_tests;
#[cfg(test)]
mod test_util;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transcript_tests;

use std::ffi::OsString;

pub use error::CliError;
pub use run::{Local, Outcome, run, run_with};

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
            Ok((args, cwd, dc_ops::Registry::devcast()?))
        });
    match prepared {
        Ok((args, cwd, registry)) => run(args, &cwd, &registry),
        Err(error) => Outcome::failure(&error, false),
    }
}
