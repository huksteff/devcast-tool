//! Built-in commands of `devcast` as clap subcommands: `about` is the line in `devcast help`,
//! `after_help` the example. Everything else is an op of the registry (`Builtin::Op`); groups
//! like `clip add` are in `builtin_groups.rs`.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::builtin_groups::{
    AudioCommand, CaptionsCommand, ClipCommand, DefaultsCommand, FontCommand, ImageCommand,
    StyleCommand, ZoomCommand,
};
use crate::builtin_program::Program;
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
use crate::{batch, events, projects, silences, skill, sources, still, style, transcript};

/// `ls projects`: the recent projects and those of the recordings folder.
pub(crate) const PROJECTS: &str = "projects";

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
        about = "Сохранить несохранённые изменения редактора в project.json; --as <имя> — копия проекта с записями рядом",
        after_help = "save --as Урок 2 копия --open"
    )]
    Save(project::SaveArgs),
    #[command(
        about = "Отбросить несохранённые изменения: проект вернётся к project.json, история отмен очистится",
        after_help = "discard"
    )]
    Discard,
    #[command(
        about = "Открыть проект в редакторе запущенной программы DevCast; несохранённое другого проекта редактор спросит в окне",
        after_help = "open --project \"Урок 2.devcast\""
    )]
    Open,
    #[command(
        about = "Закрыть окно редактора с этим проектом, вернётся HUD; несохранённое — только с --save или --discard",
        after_help = "close --save"
    )]
    Close(project::CloseArgs),
    #[command(
        about = "Переименовать проект: папка станет <имя>.devcast там же; отмены нет; в редакторе — через него, не во время экспорта",
        after_help = "rename Урок 2"
    )]
    Rename {
        #[arg(
            required = true,
            help = "Новое имя без .devcast; слова через пробел — одно имя"
        )]
        name: Vec<String>,
    },
    #[command(
        about = "Переместить папки проектов в корзину Windows (не удаляет); --restore — вернуть их оттуда на место",
        after_help = "trash \"Урок 1.devcast\" черновик.devcast"
    )]
    Trash(project::TrashArgs),
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
            help = "Список: clips, zooms …; sources — мониторы, окна и микрофоны для devcast record; projects — последние проекты и проекты папки записей"
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
        about = "События записей по готовому видео: клики, нажатые клавиши, отрезки активности",
        after_help = "events clicks --from 10 --to 1:20"
    )]
    Events {
        #[arg(
            value_enum,
            help = "clicks — нажатия кнопок мыши, keys — нажатые клавиши (и текст), activity — отрезки, где был ввод (паузы до 1 с внутри)"
        )]
        kind: events::Kind,
        #[arg(long, help = "Только события с этого момента")]
        from: Option<String>,
        #[arg(long, help = "Только события раньше этого момента")]
        to: Option<String>,
    },
    #[command(
        about = "Новый проект из видео mp4/mov/mkv/webm: копии в media/, по клипу на файл; --each — проект на каждое видео; без видео — пустой",
        after_help = "new демо.mp4 вступление.mov -o урок.devcast"
    )]
    New(project::NewArgs),
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
    /// `font add`, `font ls`: the project's own fonts.
    #[command(disable_help_subcommand = true)]
    Font {
        #[command(subcommand)]
        command: FontCommand,
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
    /// `config`, `hotkeys`, `models`, `app`, `recipes`, `beautify` (`builtin_program.rs`).
    #[command(flatten)]
    Program(Program),
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
        about = "Записать экран в новый проект; --app — в запущенной программе; record stop|pause|resume|cancel — управлять идущей записью",
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
        about = "Экспорт в MP4 или GIF через очередь в фоне; export status — очереди devcast (e2) и программы (p2), export cancel [e2|p2] — отмена",
        after_help = "export -o урок.mp4 --res 1080p --fps 60 --quality high"
    )]
    Export {
        #[arg(
            help = "status — что в очереди, cancel — отменить; без него — экспортировать и дождаться файла"
        )]
        action: Option<Action>,
        #[arg(
            help = "Номер экспорта для cancel: e2 (очередь devcast) или p2 (программы); без него cancel отменяет все"
        )]
        id: Option<String>,
        #[command(flatten)]
        args: ExportArgs,
    },
    #[command(
        about = "skill install [--agent claude|codex] — установить скилл; skill files — папка со скиллом для любой нейронки и просьба установить его",
        after_help = "skill files"
    )]
    Skill {
        #[arg(help = "Что сделать: install или files")]
        action: SkillAction,
        #[arg(long, value_parser = ["claude", "codex"], help = "Для кого установить скилл (по умолчанию claude)")]
        agent: Option<String>,
    },
    /// An op from the registry, e.g. `zoom add --at 12.5`.
    #[command(external_subcommand)]
    Op(Vec<String>),
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum SkillAction {
    Install,
    Files,
}

impl Cli {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self.command {
            Builtin::Help { command } => help(&command, ctx.registry),
            Builtin::Apply { plan } => edit::plan(ctx, &plan),
            Builtin::Undo { steps } => edit::travel(ctx, Way::Undo, steps),
            Builtin::Redo => edit::travel(ctx, Way::Redo, 1),
            Builtin::History => edit::history(ctx),
            command @ (Builtin::Save(_)
            | Builtin::Discard
            | Builtin::Open
            | Builtin::Close(_)
            | Builtin::Rename { .. }
            | Builtin::Trash(_)
            | Builtin::New(_)) => project::execute(command, ctx),
            Builtin::Info => ls::info(ctx),
            Builtin::Ls {
                list,
                from,
                to,
                fields,
                limit,
                offset,
            } => listed(
                ctx,
                Query {
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
            Builtin::Clip { command } => command.execute(ctx),
            Builtin::Zoom { command } => command.execute(ctx),
            Builtin::Captions { command } => command.execute(ctx),
            Builtin::Audio { command } => command.execute(ctx),
            Builtin::Image { command } => command.execute(ctx),
            Builtin::Font { command } => command.execute(ctx),
            Builtin::Set { assignments } => style::set(ctx, &assignments),
            Builtin::Defaults { command } => command.execute(ctx),
            Builtin::Style { command } => command.execute(ctx),
            Builtin::Program(program) => program.execute(ctx),
            Builtin::Batch(args) => batch::batch(ctx, &args),
            Builtin::Frame(args) => still::frame(ctx, &args),
            Builtin::Sheet(args) => still::sheet(ctx, &args),
            Builtin::Record { control, args } => record::record(ctx, control, &args),
            Builtin::Export { action, id, args } => {
                let channel = export::run_channel(ctx)?;
                export::export(ctx, action, id.as_deref(), &args, &channel)
            }
            Builtin::Skill {
                action: SkillAction::Install,
                agent,
            } => skill::install(&skill::target(agent.as_deref())?, ctx.globals.dry_run),
            Builtin::Skill {
                action: SkillAction::Files,
                agent: None,
            } => skill::files(ctx),
            Builtin::Skill { .. } => Err(CliError::Usage(
                "--agent выбирает нейронку для skill install; skill files подходит всем".to_owned(),
            )),
            Builtin::Op(words) => edit::op(ctx, &words),
        }
    }
}

#[path = "builtin_batch.rs"]
mod batch_rules;

#[path = "builtin_project.rs"]
mod project;

/// `ls <list>`: the lists of the project, or `sources` and `projects`, which have no time.
fn listed(ctx: &Ctx<'_>, query: Query) -> Result<Answer, CliError> {
    let special = [sources::SOURCES, PROJECTS];
    if !special.contains(&query.list.as_str()) {
        return ls::ls(ctx, &query);
    }
    if query.from.is_some() || query.to.is_some() || query.fields.is_some() {
        return Err(CliError::Usage(format!(
            "у ls {0} нет --from, --to и --fields: devcast ls {0}",
            query.list
        )));
    }
    match query.list.as_str() {
        PROJECTS => projects::ls(ctx),
        _ => sources::ls(),
    }
}
