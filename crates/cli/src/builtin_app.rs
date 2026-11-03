//! The built-in commands about the app rather than a project: its settings (`config`), its
//! shortcuts (`hotkeys`) and the speech models (`models`). Everything the settings window does
//! is a command (owner's rule 2026-11-02).

use clap::Subcommand;
use dc_settings::SpeechModel;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::run::Answer;
use crate::{app_control, config, feedback, hotkeys, models};

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigCommand {
    #[command(
        about = "Настройки программы DevCast строками ключ=значение: папка записей, запись, отсчёт, HUD, обновления, субтитры",
        after_help = "config show"
    )]
    Show,
    #[command(
        about = "Изменить настройки программы; запущенная программа применяет их сразу",
        after_help = "config set recording.fps=30 recording.countdown=0 captions.language=ru"
    )]
    Set {
        #[arg(
            required = true,
            help = "ключ=значение; ключи и значения — devcast config show, неверный ключ перечислит все"
        )]
        assignments: Vec<String>,
    },
    #[command(
        about = "Вернуть настройкам программы значения по умолчанию: названные ключи или все",
        after_help = "config reset recording.fps"
    )]
    Reset {
        #[arg(help = "Ключи из devcast config show; без них — все")]
        keys: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum HotkeysCommand {
    #[command(
        about = "Горячие клавиши: глобальные (запись) и редактора, с id, текущим и по умолчанию",
        after_help = "hotkeys ls"
    )]
    Ls,
    #[command(
        about = "Назначить сочетание по id; занятое другим — отказ, --swap меняет их местами; --clear снимает",
        after_help = "hotkeys set start_stop Ctrl+Alt+R"
    )]
    Set {
        #[arg(help = "id из devcast hotkeys ls: start_stop, save …")]
        id: String,
        #[arg(help = "Сочетание: Ctrl, Alt, Shift и клавиша (F9, Delete, OpenBracket …)")]
        keys: Option<String>,
        #[arg(long, conflicts_with = "keys", help = "Снять сочетание")]
        clear: bool,
        #[arg(
            long,
            help = "Сочетание занято: отдать его этой команде, а той — прежнее этой (как «Заменить»)"
        )]
        swap: bool,
    },
    #[command(
        about = "Вернуть сочетания по умолчанию: названные id или все с --all",
        after_help = "hotkeys reset save"
    )]
    Reset {
        #[arg(help = "id из devcast hotkeys ls")]
        ids: Vec<String>,
        #[arg(long, conflicts_with = "ids", help = "Все сочетания")]
        all: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ModelsCommand {
    #[command(
        about = "Модели распознавания речи fast и accurate: скачана ли, размер, путь",
        after_help = "models ls"
    )]
    Ls,
    #[command(
        about = "Скачать модель распознавания речи (докачивается после обрыва, проверяется SHA-256)",
        after_help = "models download accurate"
    )]
    Download {
        #[arg(value_parser = config::parse_model, help = "fast — small на процессоре; accurate — large-v3-turbo, нужна видеокарта")]
        model: SpeechModel,
    },
}

impl ConfigCommand {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self {
            Self::Show => config::show(ctx),
            Self::Set { assignments } => config::set(ctx, &assignments),
            Self::Reset { keys } => config::reset(ctx, &keys),
        }
    }
}

impl HotkeysCommand {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self {
            Self::Ls => hotkeys::ls(ctx),
            Self::Set {
                id,
                keys,
                clear,
                swap,
            } => hotkeys::set(ctx, &id, keys.as_deref(), (clear, swap)),
            Self::Reset { ids, all } => hotkeys::reset(ctx, &ids, all),
        }
    }
}

impl ModelsCommand {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self {
            Self::Ls => models::ls(ctx),
            Self::Download { model } => models::download(ctx, model),
        }
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum AppCommand {
    #[command(
        about = "Состояние программы DevCast: запись, проект редактора и распознавание субтитров, экспорт обеих очередей",
        after_help = "app status --json"
    )]
    Status,
    #[command(
        about = "Показать HUD запущенной программы (как её второй запуск)",
        after_help = "app show"
    )]
    Show,
    #[command(
        about = "Закрыть программу, как «Выход» в трее, без вопроса: идущая запись остановится и сохранится, редактор запишет рабочее состояние",
        after_help = "app quit"
    )]
    Quit,
    #[command(
        about = "Куда написать о баге или идее (как «О программе» в настройках) и папка логов, которую приложить",
        after_help = "app feedback"
    )]
    Feedback,
}

impl AppCommand {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self {
            Self::Status => app_control::status(ctx),
            Self::Show => app_control::show(ctx),
            Self::Quit => app_control::quit(ctx),
            Self::Feedback => feedback::feedback(ctx),
        }
    }
}
