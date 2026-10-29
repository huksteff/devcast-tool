//! Built-in commands of `devcast` as clap subcommands: `about` is the line in `devcast help`,
//! `after_help` the example. Everything else is an op of the registry (`Builtin::Op`).

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::edit::{self, Ctx, Way};
use crate::error::CliError;
use crate::export::{self, Action};
use crate::export_args::ExportArgs;
use crate::help::help;
use crate::ls::{self, Query};
use crate::record;
use crate::record_args::RecordArgs;
use crate::record_control::Control;
use crate::run::Answer;
use crate::{autozoom, events, import, silences, skill, sources, transcript};

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
    #[command(
        about = "Сводка проекта: длительность, сохранён ли, сколько объектов в каждом списке",
        after_help = "info"
    )]
    Info,
    #[command(
        about = "Список объектов (clips, zooms …) с временем по готовому видео",
        after_help = "ls zooms --from 10 --to 1:20 --fields scale,mode"
    )]
    Ls {
        #[arg(
            help = "Список: clips, zooms …; sources — мониторы, окна и микрофоны для devcast record"
        )]
        list: String,
        #[arg(long, help = "Только объекты, которые идут после этого момента")]
        from: Option<String>,
        #[arg(long, help = "Только объекты, которые начинаются раньше этого момента")]
        to: Option<String>,
        #[arg(long, help = "Какие поля показать, через запятую")]
        fields: Option<String>,
        #[arg(long, default_value_t = 50, help = "Сколько строк показать")]
        limit: usize,
        #[arg(long, default_value_t = 0, help = "Сколько строк пропустить")]
        offset: usize,
    },
    #[command(
        about = "Что сказано, по готовому видео: реплики или слова с id; первый раз распознаёт речь записей",
        after_help = "transcript --words --from 10 --to 30"
    )]
    Transcript(transcript::Args),
    #[command(
        about = "Тишина по готовому видео: паузы для вырезания (cut <от>-<до>)",
        after_help = "silences --min 0.8 --db -35 --from 10"
    )]
    Silences(silences::Args),
    #[command(
        about = "События записей по готовому видео: клики, сочетания клавиш, отрезки активности",
        after_help = "events clicks --from 10 --to 1:20"
    )]
    Events {
        #[arg(
            value_enum,
            help = "clicks — нажатия кнопок мыши, keys — сочетания и спецклавиши, activity — отрезки, где был ввод (паузы до 1 с внутри)"
        )]
        kind: events::Kind,
        #[arg(long, help = "Только события с этого момента")]
        from: Option<String>,
        #[arg(long, help = "Только события раньше этого момента")]
        to: Option<String>,
    },
    #[command(
        about = "Новый проект из видео mp4/mov/mkv/webm: копии в media/, по клипу на файл",
        after_help = "new демо.mp4 вступление.mov -o урок.dcproj"
    )]
    New {
        #[arg(
            required = true,
            help = "Видео mp4, mov, mkv или webm; клипы идут в этом порядке"
        )]
        videos: Vec<PathBuf>,
        #[arg(short = 'o', long, help = "Папка нового проекта *.dcproj")]
        out: PathBuf,
    },
    /// `clip add`; the other `clip …` commands are ops of the registry.
    #[command(disable_help_subcommand = true)]
    Clip {
        #[command(subcommand)]
        command: ClipCommand,
    },
    /// `zoom suggest`; the other `zoom …` commands are ops of the registry.
    #[command(disable_help_subcommand = true)]
    Zoom {
        #[command(subcommand)]
        command: ZoomCommand,
    },
    #[command(
        about = "Записать экран в новый проект; record stop|pause|resume|cancel — управлять идущей записью",
        after_help = "record --source screen:1 --duration 10 -o урок.dcproj"
    )]
    Record {
        #[arg(
            help = "stop, pause, resume или cancel (отмена удаляет проект записи); без него — начать запись"
        )]
        control: Option<Control>,
        #[command(flatten)]
        args: RecordArgs,
    },
    #[command(
        about = "Экспорт в MP4 или GIF через очередь в фоне; export status — очередь, export cancel [e2] — отмена",
        after_help = "export -o урок.mp4 --res 1080p --fps 60 --quality high"
    )]
    Export {
        #[arg(
            help = "status — что в очереди, cancel — отменить; без него — экспортировать и дождаться файла"
        )]
        action: Option<Action>,
        #[arg(help = "Номер экспорта для cancel: e2; без него cancel отменяет все")]
        id: Option<String>,
        #[command(flatten)]
        args: ExportArgs,
    },
    #[command(
        about = "Установить skill для Claude Code в ~/.claude/skills/devcast",
        after_help = "skill install"
    )]
    Skill {
        #[arg(help = "Что сделать: install")]
        action: SkillAction,
    },
    /// An op from the registry, e.g. `zoom add --at 12.5`.
    #[command(external_subcommand)]
    Op(Vec<String>),
}

#[derive(Debug, Subcommand)]
enum ClipCommand {
    #[command(
        about = "Добавить видео mp4/mov/mkv/webm клипами в конец главной дорожки (копия в media/)",
        after_help = "clip add демо.mp4"
    )]
    Add {
        #[arg(
            required = true,
            help = "Видео mp4, mov, mkv или webm; клипы идут в этом порядке"
        )]
        videos: Vec<PathBuf>,
    },
    /// `clip move`, `clip split` … from the registry.
    #[command(external_subcommand)]
    Op(Vec<String>),
}

#[derive(Debug, Subcommand)]
enum ZoomCommand {
    #[command(
        about = "Предложить автозумы по группам кликов, ничего не меняя; добавить их — zoom add --at clicks",
        after_help = "zoom suggest"
    )]
    Suggest,
    /// `zoom add`, `zoom set` … from the registry.
    #[command(external_subcommand)]
    Op(Vec<String>),
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum SkillAction {
    Install,
}

impl Cli {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self.command {
            Builtin::Help { command } => help(&command, ctx.registry),
            Builtin::Apply { plan } => edit::plan(ctx, &plan),
            Builtin::Undo { steps } => edit::travel(ctx, Way::Undo, steps),
            Builtin::Redo => edit::travel(ctx, Way::Redo, 1),
            Builtin::History => edit::history(ctx),
            Builtin::Save => edit::save(ctx),
            Builtin::Info => ls::info(ctx),
            Builtin::Ls {
                list,
                from,
                to,
                fields,
                ..
            } if list == sources::SOURCES => match (from, to, fields) {
                (None, None, None) => sources::ls(),
                _ => Err(CliError::Usage(
                    "у ls sources нет --from, --to и --fields: devcast ls sources".to_owned(),
                )),
            },
            Builtin::Transcript(args) => transcript::transcript(ctx, &args),
            Builtin::Silences(args) => silences::silences(ctx, &args),
            Builtin::Events { kind, from, to } => {
                events::events(ctx, kind, from.as_ref(), to.as_ref())
            }
            Builtin::New { videos, out } => import::new(ctx, &videos, &out),
            Builtin::Clip {
                command: ClipCommand::Add { videos },
            } => import::clip_add(ctx, &videos),
            Builtin::Clip {
                command: ClipCommand::Op(words),
            } => edit::op(ctx, &[vec!["clip".to_owned()], words].concat()),
            Builtin::Zoom {
                command: ZoomCommand::Suggest,
            } => autozoom::suggest(ctx),
            Builtin::Zoom {
                command: ZoomCommand::Op(words),
            } => autozoom::op(ctx, &[vec!["zoom".to_owned()], words].concat()),
            Builtin::Record { control, args } => record::record(ctx, control, &args),
            Builtin::Export { action, id, args } => {
                export::export(ctx, action, id.as_deref(), &args, &export::channel()?)
            }
            Builtin::Ls {
                list,
                from,
                to,
                fields,
                limit,
                offset,
            } => ls::ls(
                ctx,
                &Query {
                    list,
                    from,
                    to,
                    fields,
                    limit,
                    offset,
                },
            ),
            Builtin::Skill {
                action: SkillAction::Install,
            } => skill::install(&skill::target()?, ctx.globals.dry_run),
            Builtin::Op(words) => edit::op(ctx, &words),
        }
    }
}
