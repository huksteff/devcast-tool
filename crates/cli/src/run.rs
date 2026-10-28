//! One run of `devcast`: arguments in; text for stdout and stderr and an exit code out.
//! stdout gets only the result, stderr errors and warnings. Exit codes: 0 done, 1 failed,
//! 2 wrong command line.

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use dc_ops::Registry;
use serde_json::{Value, json};

use crate::edit::{self, Ctx, Way};
use crate::error::CliError;
use crate::globals::{self, Globals};
use crate::help::help;

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

#[derive(Debug, Parser)]
#[command(
    name = "devcast",
    disable_help_flag = true,
    disable_version_flag = true,
    disable_help_subcommand = true
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Builtin,
}

/// Built-in commands. `about` is the line in `devcast help`, `after_help` the example.
#[derive(Debug, Subcommand)]
enum Builtin {
    #[command(
        about = "Список команд; с именем команды — её аргументы и пример",
        after_help = "help zoom add"
    )]
    Help {
        #[arg(help = "Команда из нескольких слов, например zoom add")]
        command: Vec<String>,
    },
    #[command(
        about = "Применить план из JSON-файла: все шаги — один шаг отмены, всё или ничего",
        after_help = "apply plan.json --dry-run"
    )]
    Apply {
        #[arg(help = r#"Файл плана: [{"op": "zoom add", "at": 12.5}, …]"#)]
        plan: PathBuf,
    },
    #[command(about = "Отменить последние шаги", after_help = "undo 2")]
    Undo {
        #[arg(default_value_t = 1, help = "Сколько шагов отменить")]
        steps: usize,
    },
    #[command(about = "Вернуть последний отменённый шаг", after_help = "redo")]
    Redo,
    #[command(
        about = "Шаги, которые отменит undo и вернёт redo, новые сверху",
        after_help = "history"
    )]
    History,
    #[command(
        about = "Сохранить несохранённые изменения редактора в project.json",
        after_help = "save"
    )]
    Save,
    /// An op from the registry, e.g. `zoom add --at 12.5`.
    #[command(external_subcommand)]
    Op(Vec<String>),
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
    match cli.command {
        Builtin::Help { command } => help(&command, registry),
        Builtin::Apply { plan } => edit::plan(&ctx, &plan),
        Builtin::Undo { steps } => edit::travel(&ctx, Way::Undo, steps),
        Builtin::Redo => edit::travel(&ctx, Way::Redo, 1),
        Builtin::History => edit::history(&ctx),
        Builtin::Save => edit::save(&ctx),
        Builtin::Op(words) => edit::op(&ctx, &words),
    }
}
