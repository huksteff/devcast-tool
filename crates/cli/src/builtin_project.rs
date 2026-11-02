//! The built-in commands about the project as a whole: saving, the copy, dropping the working
//! state, the editor window, the folder name, the Recycle Bin, new projects.

use std::path::PathBuf;

use clap::Args;
use dc_ops::live::Changes;

use super::Builtin;
use crate::edit::{self, Ctx};
use crate::error::CliError;
use crate::run::Answer;
use crate::{discard, import, open, rename, save_as, trash};

#[derive(Debug, Args)]
pub(super) struct SaveArgs {
    #[arg(
        long = "as",
        num_args = 1..,
        value_name = "ИМЯ",
        help = "Копия рабочего состояния в <имя>.devcast рядом; проект, открытый в редакторе, отбрасывает несохранённое, и редактор открывает копию (как «Сохранить как…»)"
    )]
    pub as_name: Option<Vec<String>>,
    #[arg(
        long,
        requires = "as_name",
        help = "Открыть копию в редакторе запущенной программы"
    )]
    open: bool,
}

#[derive(Debug, Args)]
pub(super) struct CloseArgs {
    #[arg(
        long,
        conflicts_with = "discard",
        help = "Сначала сохранить несохранённое в project.json"
    )]
    save: bool,
    #[arg(long, help = "Сначала отбросить несохранённое, как devcast discard")]
    discard: bool,
}

#[derive(Debug, Args)]
pub(super) struct TrashArgs {
    #[arg(
        required = true,
        help = "Папки проектов *.devcast; для --restore — пути, где они были"
    )]
    projects: Vec<PathBuf>,
    #[arg(
        long,
        help = "Вернуть из корзины последние удалённые папки с этими путями"
    )]
    restore: bool,
}

#[derive(Debug, Args)]
pub(super) struct NewArgs {
    #[arg(
        help = "Видео mp4, mov, mkv или webm; клипы идут в этом порядке; без них — пустой проект, как «Новый проект»"
    )]
    videos: Vec<PathBuf>,
    #[arg(
        short = 'o',
        long,
        help = "Папка нового проекта *.devcast; без видео и -o — «Новый проект» в папке записей (devcast config show); с --each — папка, где появятся проекты (без -o — текущая)"
    )]
    out: Option<PathBuf>,
    #[arg(
        long,
        conflicts_with = "each",
        help = "Открыть новый проект в редакторе запущенной программы"
    )]
    open: bool,
    #[arg(
        long,
        help = "Проект на каждое видео: <имя видео>.devcast, занято — -2, -3 …"
    )]
    each: bool,
}

pub(super) fn execute(command: Builtin, ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    match command {
        Builtin::Save(args) => match args.as_name {
            None => edit::save(ctx),
            Some(words) => save_as::save_as(ctx, &words.join(" "), args.open),
        },
        Builtin::Discard => discard::discard(ctx),
        Builtin::Open => open::open(ctx),
        Builtin::Close(args) => {
            let changes = match (args.save, args.discard) {
                (true, _) => Some(Changes::Save),
                (false, true) => Some(Changes::Discard),
                (false, false) => None,
            };
            discard::close(ctx, changes)
        }
        Builtin::Rename { name } => rename::rename(ctx, &name),
        Builtin::Trash(args) => trash::trash(ctx, &args.projects, args.restore),
        Builtin::New(args) => match (args.each, args.out, args.videos.is_empty()) {
            (true, _, true) => Err(CliError::Usage(
                "у new --each нужны видео: devcast help new".to_owned(),
            )),
            (true, out, false) => import::new_each(ctx, &args.videos, out.as_deref()),
            (false, out, true) => import::new_empty(ctx, out.as_deref(), args.open),
            (false, Some(out), false) => import::new(ctx, &args.videos, &out, args.open),
            (false, None, false) => Err(CliError::Usage(
                "у new с видео нужен -o <папка>.devcast или --each: devcast help new".to_owned(),
            )),
        },
        _ => Err(CliError::Usage(
            "команда не про проект целиком: devcast help".to_owned(),
        )),
    }
}
