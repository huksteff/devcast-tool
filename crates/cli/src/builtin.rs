//! Built-in commands of `devcast` as clap subcommands: `about` is the line in `devcast help`,
//! `after_help` the example. Everything else is an op of the registry (`Builtin::Op`); groups
//! like `clip add` are in `builtin_groups.rs`.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::builtin_groups::{
    AudioCommand, CaptionsCommand, ClipCommand, DefaultsCommand, ImageCommand, StyleCommand,
    ZoomCommand,
};
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
use crate::still::{FrameArgs, SheetArgs};
use crate::{batch, events, silences, skill, sources, still, style, transcript};

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
        after_help = "new демо.mp4 вступление.mov -o урок.devcast"
    )]
    New {
        #[arg(
            required = true,
            help = "Видео mp4, mov, mkv или webm; клипы идут в этом порядке"
        )]
        videos: Vec<PathBuf>,
        #[arg(short = 'o', long, help = "Папка нового проекта *.devcast")]
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
    /// `captions gen`, `export`, `import`; the other `captions …` commands are ops of the registry.
    #[command(disable_help_subcommand = true)]
    Captions {
        #[command(subcommand)]
        command: CaptionsCommand,
    },
    /// `audio add`; the other `audio …` commands are ops of the registry.
    #[command(disable_help_subcommand = true)]
    Audio {
        #[command(subcommand)]
        command: AudioCommand,
    },
    /// `image set`: the picture of an image annotation.
    #[command(disable_help_subcommand = true)]
    Image {
        #[command(subcommand)]
        command: ImageCommand,
    },
    #[command(
        about = "Стиль проекта: настройки эффектов настройка.параметр=значение, все — одна запись истории",
        after_help = "set zoom.preset=smooth captions.size=36 cursor.size=3"
    )]
    Set {
        #[arg(
            required = true,
            help = "настройка.параметр=значение; параметр — аргумент команды этих настроек"
        )]
        assignments: Vec<String>,
    },
    /// `defaults show|set|from-project|reset`: settings of new projects.
    #[command(disable_help_subcommand = true)]
    Defaults {
        #[command(subcommand)]
        command: DefaultsCommand,
    },
    /// `style copy`: the look of one project in others.
    #[command(disable_help_subcommand = true)]
    Style {
        #[command(subcommand)]
        command: StyleCommand,
    },
    #[command(
        about = "Одна команда в нескольких проектах: у каждого своя запись истории; ошибка в одном не останавливает другие",
        after_help = "batch *.devcast -- set captions.size=36"
    )]
    Batch(batch::Args),
    #[command(
        about = "Кадр готового видео в PNG; --grid — сетка долей кадра для --rect и фокуса",
        after_help = "frame --at 12.5 --grid -o кадр.png"
    )]
    Frame(FrameArgs),
    #[command(
        about = "Раскадровка: n кадров отрезка готового видео в одном PNG, время кадров — в ответе",
        after_help = "sheet --from 10 --to 20 -n 6 -o раскадровка.png"
    )]
    Sheet(SheetArgs),
    #[command(
        about = "Записать экран в новый проект; record stop|pause|resume|cancel — управлять идущей записью",
        after_help = "record --source screen:1 --duration 10 -o урок.devcast"
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
            Builtin::Transcript(args) => transcript::transcript(ctx, &args),
            Builtin::Silences(args) => silences::silences(ctx, &args),
            Builtin::Events { kind, from, to } => {
                events::events(ctx, kind, from.as_ref(), to.as_ref())
            }
            Builtin::New { videos, out } => crate::import::new(ctx, &videos, &out),
            Builtin::Clip { command } => command.execute(ctx),
            Builtin::Zoom { command } => command.execute(ctx),
            Builtin::Captions { command } => command.execute(ctx),
            Builtin::Audio { command } => command.execute(ctx),
            Builtin::Image { command } => command.execute(ctx),
            Builtin::Set { assignments } => style::set(ctx, &assignments),
            Builtin::Defaults { command } => command.execute(ctx),
            Builtin::Style { command } => command.execute(ctx),
            Builtin::Batch(args) => batch::batch(ctx, &args),
            Builtin::Frame(args) => still::frame(ctx, &args),
            Builtin::Sheet(args) => still::sheet(ctx, &args),
            Builtin::Record { control, args } => record::record(ctx, control, &args),
            Builtin::Export { action, id, args } => {
                let channel = match ctx.local.export_channel {
                    Some(channel) => channel.to_owned(),
                    None => export::channel()?,
                };
                export::export(ctx, action, id.as_deref(), &args, &channel)
            }
            Builtin::Skill {
                action: SkillAction::Install,
            } => skill::install(&skill::target()?, ctx.globals.dry_run),
            Builtin::Op(words) => edit::op(ctx, &words),
        }
    }

    /// Why this command cannot run inside `batch` (it works on one project there): `None` when
    /// it can.
    pub(crate) fn not_for_batch(&self) -> Option<&'static str> {
        match &self.command {
            Builtin::Help { .. } | Builtin::Skill { .. } | Builtin::Defaults { .. } => {
                Some("не работает с проектом")
            }
            Builtin::Ls { list, .. } if list == sources::SOURCES => Some("не работает с проектом"),
            Builtin::New { .. } | Builtin::Record { .. } => Some("создаёт новый проект"),
            Builtin::Batch(_) | Builtin::Style { .. } => Some("сама перечисляет проекты"),
            Builtin::Export { .. } | Builtin::Frame(_) | Builtin::Sheet(_) => Some(
                "пишет один файл -o на все проекты; запусти её для каждого проекта с --project",
            ),
            Builtin::Captions {
                command: CaptionsCommand::Export { .. },
            } => Some(
                "пишет один файл -o на все проекты; запусти её для каждого проекта с --project",
            ),
            _ => None,
        }
    }

    /// The words of the registry op this command line names (`clip move c2 --to 1`), so `batch`
    /// can check the name and the arguments once before the projects.
    pub(crate) fn op_words(&self) -> Option<Vec<String>> {
        let (group, words) = match &self.command {
            Builtin::Op(words) => return Some(words.clone()),
            Builtin::Clip {
                command: ClipCommand::Op(words),
            } => ("clip", words),
            Builtin::Zoom {
                command: ZoomCommand::Op(words),
            } => ("zoom", words),
            Builtin::Captions {
                command: CaptionsCommand::Op(words),
            } => ("captions", words),
            Builtin::Audio {
                command: AudioCommand::Op(words),
            } => ("audio", words),
            _ => return None,
        };
        Some([vec![group.to_owned()], words.clone()].concat())
    }
}
