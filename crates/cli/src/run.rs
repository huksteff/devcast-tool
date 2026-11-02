//! One run of `devcast`: arguments in; text for stdout and stderr and an exit code out.
//! stdout gets only the result, stderr errors and warnings. Exit codes: 0 done, 1 failed,
//! 2 wrong command line.

use std::path::Path;

use clap::Parser;
use dc_ops::Registry;
use serde_json::{Value, json};

use crate::builtin::Cli;
use crate::edit::Ctx;
use crate::error::CliError;
use crate::globals;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub code: u8,
    pub stdout: String,
    pub stderr: String,
}

impl Outcome {
    pub fn failure(error: &CliError, json: bool) -> Self {
        let text = match json {
            true => json!({"error": error.to_string(), "code": error.code()}).to_string(),
            false => error.to_string(),
        };
        Self {
            code: error.code(),
            stdout: String::new(),
            stderr: text + "\n",
        }
    }
}

/// What a successful command prints: `text` normally, `json` under `--json`.
pub(crate) struct Answer {
    pub text: String,
    pub json: Value,
    pub warnings: Vec<String>,
    /// Exit code: 0, or 1 when a part of the work failed (`batch`: some projects did not take
    /// the command); the failures are among `warnings`.
    pub code: u8,
}

impl Answer {
    pub(crate) fn new(text: String, json: Value) -> Self {
        Self {
            text,
            json,
            warnings: Vec::new(),
            code: 0,
        }
    }
}

pub fn run(args: Vec<String>, cwd: &Path, registry: &Registry) -> Outcome {
    run_with(args, cwd, registry, Local::default())
}

/// What a run uses outside the project folder. Tests give their own, so the real ones are never
/// touched.
#[derive(Debug, Default, Clone, Copy)]
pub struct Local<'a> {
    /// The app defaults file; `None` — the real one in the settings folder.
    pub defaults: Option<&'a Path>,
    /// The channel of the export queue; `None` — the real `devcast-export-<user>`.
    pub export_channel: Option<&'a str>,
    /// The channel of the app whose editor may hold the project; `None` — the real
    /// `devcast-app-<user>`.
    pub app_channel: Option<&'a str>,
}

/// [`run`] with `local` instead of the real files and channels (tests).
pub fn run_with(args: Vec<String>, cwd: &Path, registry: &Registry, local: Local<'_>) -> Outcome {
    let (globals, rest) = globals::split(args);
    let ctx = Ctx {
        globals: &globals,
        cwd,
        registry,
        local,
    };
    match rest.and_then(|rest| dispatch(rest, &ctx)) {
        Ok(answer) => Outcome {
            code: answer.code,
            stdout: match globals.json {
                true => answer.json.to_string(),
                false => answer.text,
            } + "\n",
            stderr: answer
                .warnings
                .iter()
                .map(|w| format!("внимание: {w}\n"))
                .collect(),
        },
        Err(error) => Outcome::failure(&error, globals.json),
    }
}

/// The built-in command or op `rest` names, run in `ctx`.
pub(crate) fn dispatch(rest: Vec<String>, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    parse(rest)?.execute(ctx)
}

pub(crate) fn parse(rest: Vec<String>) -> Result<Cli, CliError> {
    if rest.is_empty() {
        return Err(CliError::Usage(
            "нужна команда, например: devcast zoom add --at 12.5. Список: devcast help".to_owned(),
        ));
    }
    Cli::try_parse_from(std::iter::once("devcast".to_owned()).chain(rest)).map_err(|e| {
        CliError::Usage(format!(
            "командная строка не разобрана ({}). Список: devcast help",
            e.kind()
        ))
    })
}
