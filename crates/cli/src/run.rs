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
use crate::globals::{self, Globals};

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
}

impl Answer {
    pub(crate) fn new(text: String, json: Value) -> Self {
        Self {
            text,
            json,
            warnings: Vec::new(),
        }
    }
}

pub fn run(args: Vec<String>, cwd: &Path, registry: &Registry) -> Outcome {
    let (globals, rest) = globals::split(args);
    match rest.and_then(|rest| dispatch(rest, &globals, cwd, registry)) {
        Ok(answer) => Outcome {
            code: 0,
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

fn dispatch(
    rest: Vec<String>,
    globals: &Globals,
    cwd: &Path,
    registry: &Registry,
) -> Result<Answer, CliError> {
    if rest.is_empty() {
        return Err(CliError::Usage(
            "нужна команда, например: devcast zoom add --at 12.5. Список: devcast help".to_owned(),
        ));
    }
    let cli =
        Cli::try_parse_from(std::iter::once("devcast".to_owned()).chain(rest)).map_err(|e| {
            CliError::Usage(format!(
                "командная строка не разобрана ({}). Список: devcast help",
                e.kind()
            ))
        })?;
    let ctx = Ctx {
        globals,
        cwd,
        registry,
    };
    cli.execute(&ctx)
}
