//! Groups of built-in commands (`clip add`, `defaults show` …): the first word is a clap command,
//! the second its subcommand. Words a group does not know go to the registry of ops.

use std::path::PathBuf;

use clap::Subcommand;

use crate::edit::{self, Ctx};
use crate::error::CliError;
use crate::run::Answer;
use crate::{audio, autozoom, captions, defaults, font, image, import, style};

#[derive(Debug, Subcommand)]
pub(crate) enum ClipCommand {
    #[command(
        about = "Добавить видео mp4/mov/mkv/webm или записи (папки *.devcast) клипами в конец главной дорожки (копия в media/)",
        after_help = "clip add демо.mp4 ../recording-2.devcast"
    )]
    Add {
        #[arg(
            required = true,
            help = "Видео mp4, mov, mkv или webm либо папка проекта *.devcast: его записи копируются с курсором, клавишами и звуком; клипы идут в этом порядке"
        )]
        videos: Vec<PathBuf>,
    },
    /// `clip move`, `clip split` … from the registry.
    #[command(external_subcommand)]
    Op(Vec<String>),
}

#[derive(Debug, Subcommand)]
pub(crate) enum ZoomCommand {
    #[command(
        about = "Предложить автозумы по группам кликов, ничего не меняя; добавить их — zoom add --at clicks",
        after_help = "zoom suggest"
    )]
    Suggest,
    /// `zoom add`, `zoom set` … from the registry.
    #[command(external_subcommand)]
    Op(Vec<String>),
}

#[derive(Debug, Subcommand)]
pub(crate) enum CaptionsCommand {
    #[command(
        about = "Субтитры из речи записей: заменяет все реплики; первый раз распознаёт речь, как transcript",
        after_help = "captions gen --tracks mic,system --language ru"
    )]
    Gen(captions::GenArgs),
    #[command(
        about = "Остановить «Создать субтитры», идущее в редакторе с этим проектом (реплики не меняются)",
        after_help = "captions stop"
    )]
    Stop,
    #[command(
        about = "Текст всех реплик в файл для перевода: строка «s3 текст» на реплику, по порядку видео",
        after_help = "captions export -o субтитры.txt"
    )]
    Export {
        #[arg(short = 'o', long, help = "Текстовый файл, куда писать")]
        out: PathBuf,
    },
    #[command(
        about = "Новый текст реплик из файла captions export; время реплик не меняется",
        after_help = "captions import субтитры.txt"
    )]
    Import {
        #[arg(help = "Файл со строками «s3 текст»; реплики, которых в нём нет, не меняются")]
        file: PathBuf,
    },
    /// `captions set`, `captions split` … from the registry.
    #[command(external_subcommand)]
    Op(Vec<String>),
}

#[derive(Debug, Subcommand)]
pub(crate) enum ImageCommand {
    #[command(
        about = "Картинка аннотации image: файл копируется в assets/images/, вписывается в бокс с сохранением пропорций (с --at — в ключе)",
        after_help = "image set a3 --file логотип.png --at 4.5"
    )]
    Set(image::SetArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum FontCommand {
    #[command(
        about = "Добавить свой шрифт .ttf/.otf: файл копируется в assets/fonts/, стиль выбирает его по семейству",
        after_help = "font add шрифт.ttf"
    )]
    Add(font::AddArgs),
    #[command(
        about = "Шрифты, которые можно выбрать в тексте и субтитрах: встроенные и добавленные в проект",
        after_help = "font ls"
    )]
    Ls,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AudioCommand {
    #[command(
        about = "Добавить звук mp3/wav/aac/m4a/flac/ogg по времени готового видео (копия в assets/audio/)",
        after_help = "audio add музыка.mp3 --at 5 --duration 30"
    )]
    Add(audio::AddArgs),
    /// `audio channel`, `audio set` … from the registry.
    #[command(external_subcommand)]
    Op(Vec<String>),
}

#[derive(Debug, Subcommand)]
pub(crate) enum DefaultsCommand {
    #[command(
        about = "Настройки по умолчанию для новых проектов и записей: что отличается от заводских",
        after_help = "defaults show"
    )]
    Show,
    #[command(
        about = "Изменить настройки по умолчанию, как devcast set: настройка.параметр=значение",
        after_help = "defaults set zoom.preset=smooth captions.size=36"
    )]
    Set {
        #[arg(
            required = true,
            help = "настройка.параметр=значение: настройки и их команды — devcast help set"
        )]
        assignments: Vec<String>,
    },
    #[command(
        about = "Сделать настройки этого проекта настройками по умолчанию (содержимое не берётся)",
        after_help = "defaults from-project"
    )]
    FromProject,
    #[command(
        about = "Вернуть заводские настройки по умолчанию",
        after_help = "defaults reset"
    )]
    Reset,
}

#[derive(Debug, Subcommand)]
pub(crate) enum StyleCommand {
    #[command(
        about = "Скопировать стиль (настройки эффектов, без содержимого) из проекта в другие: у каждого одна запись истории",
        after_help = "style copy --from урок.devcast --to *.devcast"
    )]
    Copy(style::CopyArgs),
}

impl ClipCommand {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self {
            Self::Add { videos } => import::clip_add(ctx, &videos),
            Self::Op(words) => edit::op(ctx, &[vec!["clip".to_owned()], words].concat()),
        }
    }
}

impl ZoomCommand {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self {
            Self::Suggest => autozoom::suggest(ctx),
            Self::Op(words) => edit::op(ctx, &[vec!["zoom".to_owned()], words].concat()),
        }
    }
}

impl CaptionsCommand {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self {
            Self::Gen(args) => captions::generate(ctx, &args),
            Self::Stop => captions::stop(ctx),
            Self::Export { out } => captions::export(ctx, &out),
            Self::Import { file } => captions::import(ctx, &file),
            Self::Op(words) => edit::op(ctx, &[vec!["captions".to_owned()], words].concat()),
        }
    }
}

impl ImageCommand {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self {
            Self::Set(args) => image::set(ctx, &args),
        }
    }
}

impl FontCommand {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self {
            Self::Add(args) => font::add(ctx, &args),
            Self::Ls => font::ls(ctx),
        }
    }
}

impl AudioCommand {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self {
            Self::Add(args) => audio::add(ctx, &args),
            Self::Op(words) => edit::op(ctx, &[vec!["audio".to_owned()], words].concat()),
        }
    }
}

impl DefaultsCommand {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self {
            Self::Show => defaults::show(ctx),
            Self::Set { assignments } => defaults::set(ctx, &assignments),
            Self::FromProject => defaults::from_project(ctx),
            Self::Reset => defaults::reset(ctx),
        }
    }
}

impl StyleCommand {
    pub(crate) fn execute(self, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
        match self {
            Self::Copy(args) => style::copy(ctx, &args),
        }
    }
}
