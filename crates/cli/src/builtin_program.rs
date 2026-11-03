//! The built-in commands about the app and the user rather than a project, flattened into
//! [`crate::builtin::Builtin`]: settings, shortcuts, speech models, the running app, and
//! «Сделать красиво» — the recipes and the run of the user's AI agent (8.11).

use clap::Subcommand;

use crate::beautify::{self, BeautifyArgs};
use crate::builtin_app::{AppCommand, ConfigCommand, HotkeysCommand, ModelsCommand};
use crate::edit::Ctx;
use crate::error::CliError;
use crate::recipes::{self, RecipesCommand};
use crate::run::Answer;

#[derive(Debug, Subcommand)]
pub(crate) enum Program {
    /// `config show|set|reset`: the settings of the app.
    #[command(disable_help_subcommand = true)]
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// `hotkeys ls|set|reset`: the global and the editor shortcuts.
    #[command(disable_help_subcommand = true)]
    Hotkeys {
        #[command(subcommand)]
        command: HotkeysCommand,
    },
    /// `models ls|download`: the speech recognition models.
    #[command(disable_help_subcommand = true)]
    Models {
        #[command(subcommand)]
        command: ModelsCommand,
    },
    /// `app show|quit|status`: the running app.
    #[command(disable_help_subcommand = true)]
    App {
        #[command(subcommand)]
        command: AppCommand,
    },
    /// `recipes ls|add|rm|check|agent|model`: what «Сделать красиво» asks the AI agent.
    #[command(disable_help_subcommand = true)]
    Recipes {
        #[command(subcommand)]
        command: RecipesCommand,
    },
    #[command(
        about = "«Сделать красиво»: нейронка пользователя (Claude Code или Codex) монтирует проект командами devcast по отмеченным рецептам; всё одним шагом отмены",
        after_help = "beautify --recipe pauses --recipe captions --ask \"не режь паузу на 0:42\""
    )]
    Beautify(BeautifyArgs),
}

impl Program {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self {
            Self::Config { command } => command.execute(ctx),
            Self::Hotkeys { command } => command.execute(ctx),
            Self::Models { command } => command.execute(ctx),
            Self::App { command } => command.execute(ctx),
            Self::Recipes { command } => recipes::execute(ctx, command),
            Self::Beautify(args) => beautify::beautify(ctx, &args),
        }
    }

    /// Why it is not for `batch`: none of them works on one project at a time, except
    /// `beautify`, which runs a long agent per project.
    pub(crate) fn not_for_batch(&self) -> &'static str {
        match self {
            Self::Beautify(_) => "запускает нейронку: проекты по одному",
            _ => "не работает с проектом",
        }
    }
}
