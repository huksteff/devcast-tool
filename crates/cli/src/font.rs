//! `devcast font add <file>` copies a `.ttf`/`.otf` file into the project's `assets/fonts/`
//! (`dc_ops::import_font`, shared with the editor); `devcast font ls` lists the families a style
//! can name. A font file is not a change of `project.json`: there is no history entry, the style
//! that names the family is the undoable change. A project the editor holds hears of the font
//! (`Request::Fonts`): its font pickers list it at once.

use std::path::PathBuf;

use serde_json::json;

use crate::edit::{self, Ctx};
use crate::error::CliError;
use crate::run::Answer;

#[derive(Debug, clap::Args)]
pub(crate) struct AddArgs {
    #[arg(help = "Файл шрифта .ttf или .otf; копируется в assets/fonts/ проекта")]
    file: PathBuf,
}

pub(crate) fn add(ctx: &Ctx<'_>, args: &AddArgs) -> Result<Answer, CliError> {
    let source = ctx.cwd.join(&args.file);
    let mut workspace = ctx.open(true)?;
    let imported = dc_ops::import_font(workspace.dir(), &source, ctx.globals.dry_run)
        .map_err(|e| CliError::Failed(e.to_string()))?;
    if let Some(editor) = workspace.editor().filter(|_| !ctx.globals.dry_run) {
        editor.fonts_added()?;
    }
    let (family, file) = (&imported.family, &imported.file);
    let already = !file.copied && !ctx.globals.dry_run;
    let text = format!(
        "{}шрифт «{family}»: {}{}",
        ctx.prefix(),
        file.path,
        if already {
            " (уже был в проекте)"
        } else {
            ""
        }
    );
    let json = json!({
        "family": family,
        "path": file.path,
        "copied": file.copied,
        "dry_run": ctx.globals.dry_run,
    });
    Ok(edit::answer(&mut workspace, text, json))
}

pub(crate) fn ls(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let mut workspace = ctx.open(false)?;
    let own = dc_ops::fonts::project_fonts(workspace.dir().root())
        .map_err(|e| CliError::Failed(e.to_string()))?;
    let mut lines: Vec<String> = dc_render::FAMILIES
        .iter()
        .map(|f| format!("{f}  встроенный"))
        .collect();
    lines.extend(own.iter().map(|(family, path)| format!("{family}  {path}")));
    let mut fonts: Vec<_> = dc_render::FAMILIES
        .iter()
        .map(|f| json!({"family": f, "path": null}))
        .collect();
    fonts.extend(
        own.iter()
            .map(|(family, path)| json!({"family": family, "path": path})),
    );
    Ok(edit::answer(&mut workspace, lines.join("\n"), json!(fonts)))
}
