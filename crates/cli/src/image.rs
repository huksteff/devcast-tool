//! `devcast image set <id> --file <picture> [--at t]`: the picture is read and copied into the
//! project's `assets/images/` and `ImageSet` puts it into the annotation as one undo step
//! (`dc_ops::import_picture`, shared with the editor). A failed change deletes the file it copied.

use std::path::PathBuf;

use dc_fx_annotations::AnnotId;
use dc_fx_image::ImageSet;

use crate::edit::{self, Ctx};
use crate::error::CliError;
use crate::run::Answer;

#[derive(Debug, clap::Args)]
pub(crate) struct SetArgs {
    #[arg(help = "Аннотация-картинка: a3 (devcast annot add image создаёт её)")]
    id: String,
    #[arg(
        long,
        help = "Картинка PNG, JPEG, WebP или GIF (анимация проигрывается); копируется в assets/images/ проекта"
    )]
    file: PathBuf,
    #[arg(
        long,
        help = "Момент готового видео: картинка меняется в ключе этого момента; без него — везде"
    )]
    at: Option<String>,
}

pub(crate) fn set(ctx: &Ctx<'_>, args: &SetArgs) -> Result<Answer, CliError> {
    let id: AnnotId = args
        .id
        .parse()
        .map_err(|e| CliError::Usage(format!("{e}: укажи id аннотации, например a3")))?;
    let at = crate::ls::time(args.at.as_ref(), "--at")?;
    let source = ctx.cwd.join(&args.file);
    let mut workspace = ctx.open(true)?;
    let imported = dc_ops::import_picture(workspace.dir(), &source, (id, at), ctx.globals.dry_run)
        .map_err(|e| CliError::Failed(e.to_string()))?;
    let command = &imported.command;
    let answer = edit::apply_commands(ctx, &mut workspace, ImageSet::NAME, &[command]);
    if let Err(error) = &answer {
        imported
            .discard()
            .map_err(|e| CliError::Failed(format!("{error}; {e}")))?;
    }
    answer
}
