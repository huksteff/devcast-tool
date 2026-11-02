//! `devcast new <video...> -o name.devcast` (`dc_ops::new_project`, with `--each` a project per
//! video: `dc_ops::new_projects`) and `devcast clip add
//! <video|project...>` (`dc_ops::import_clips`: videos and the recordings of DevCast projects are
//! copied into `media/`), then clips at the end of the main track through the command `clip add`
//! (`dc_fx_cut::ClipAdd`).

use std::path::{Path, PathBuf};

use dc_core::Project;
use dc_fx_cut::ClipAdd;
use dc_ops::{Applied, as_commands};
use dc_project::Defaults;
use dc_project::dir::MEDIA_DIR;

use crate::edit::{self, Ctx, applied_json};
use crate::error::CliError;
use crate::find::new_folder;
use crate::run::Answer;

/// `devcast clip add <video|project...>`: one undo step for all of them; refused commands delete
/// the copies.
pub(crate) fn clip_add(ctx: &Ctx<'_>, sources: &[PathBuf]) -> Result<Answer, CliError> {
    let sources = paths(ctx.cwd, sources);
    let mut workspace = ctx.open(true)?;
    let import = dc_ops::import_clips(workspace.dir(), &sources, ctx.globals.dry_run)
        .map_err(|e| CliError::Failed(e.to_string()))?;
    let applied = edit::apply_commands(
        ctx,
        &mut workspace,
        ClipAdd::NAME,
        &as_commands(&import.commands),
    );
    if applied.is_err() {
        import
            .discard()
            .map_err(|e| CliError::Failed(e.to_string()))?;
    }
    applied
}

/// `devcast new <video...> -o name.devcast`: a project with the app defaults and a clip per video.
pub(crate) fn new(
    ctx: &Ctx<'_>,
    videos: &[PathBuf],
    out: &Path,
    open: bool,
) -> Result<Answer, CliError> {
    let root = new_folder(ctx.cwd, out)?;
    let sources = paths(ctx.cwd, videos);
    let defaults = Defaults::load(&ctx.defaults_file()?)?;
    if ctx.globals.dry_run {
        let mut project = Project::default();
        defaults.apply(&mut project);
        let media = dc_media::import_dry_run(&root.join(MEDIA_DIR), &sources)
            .map_err(|e| CliError::Failed(e.to_string()))?;
        let applied = dc_ops::add_clips(&mut project, media)?;
        return Ok(created(&root, &applied, true));
    }
    let applied = dc_ops::new_project(&root, &sources, &defaults, APP)
        .map_err(|e| CliError::Failed(e.to_string()))?;
    opened(ctx, created(&root, &applied, false), &root, open)
}

/// Who holds a new project while it is made.
const APP: &str = "devcast new";

/// `devcast new [-o name.devcast] [--open]` without videos: an empty project with the app
/// defaults, as «Новый проект» of the editor; without `-o` in the recordings folder of the app
/// settings, named as the editor names it (`dc_ops::empty_root`).
pub(crate) fn new_empty(ctx: &Ctx<'_>, out: Option<&Path>, open: bool) -> Result<Answer, CliError> {
    let defaults = Defaults::load(&ctx.defaults_file()?)?;
    let folder = ctx.app_settings()?.recordings_dir;
    let root = match out {
        Some(out) => new_folder(ctx.cwd, out)?,
        None => dc_ops::empty_root(&folder),
    };
    if ctx.globals.dry_run {
        let mut project = Project::default();
        defaults.apply(&mut project);
        return Ok(created(
            &root,
            &dc_ops::add_clips(&mut project, Vec::new())?,
            true,
        ));
    }
    let (root, applied) = match out {
        Some(_) => {
            let applied = dc_ops::new_project(&root, &[], &defaults, APP);
            (root, applied.map_err(|e| CliError::Failed(e.to_string()))?)
        }
        None => dc_ops::new_empty(&folder, &defaults, APP)
            .map_err(|e| CliError::Failed(e.to_string()))?,
    };
    opened(ctx, created(&root, &applied, false), &root, open)
}

/// With `--open` the running app opens the new project in its editor.
fn opened(ctx: &Ctx<'_>, mut answer: Answer, root: &Path, open: bool) -> Result<Answer, CliError> {
    if !open || ctx.globals.dry_run {
        return Ok(answer);
    }
    let asking = crate::open::open_in_app(ctx, &crate::open::absolute(root))?;
    answer.text.push_str(match asking {
        false => "; открыт в редакторе",
        true => "; откроется в редакторе после ответа о несохранённом в его окне",
    });
    answer.json["opened"] = (!asking).into();
    Ok(answer)
}

fn paths(cwd: &Path, sources: &[PathBuf]) -> Vec<PathBuf> {
    sources.iter().map(|source| cwd.join(source)).collect()
}

fn created(root: &Path, applied: &Applied, dry_run: bool) -> Answer {
    let prefix = match dry_run {
        true => "dry-run, проект не создан: ",
        false => "создан ",
    };
    let mut json = applied_json(applied);
    json["project"] = root.display().to_string().into();
    json["dry_run"] = dry_run.into();
    Answer::new(format!("{prefix}{}: {applied}", root.display()), json)
}

/// `devcast new --each <video...> [-o folder]`: a project per video in `folder` (the current
/// folder without it), named as «Импорт…» of the library names them (`dc_ops::project_roots`).
pub(crate) fn new_each(
    ctx: &Ctx<'_>,
    videos: &[PathBuf],
    folder: Option<&Path>,
) -> Result<Answer, CliError> {
    let folder = folder.map_or_else(|| ctx.cwd.to_path_buf(), |f| ctx.cwd.join(f));
    let sources = paths(ctx.cwd, videos);
    let defaults = Defaults::load(&ctx.defaults_file()?)?;
    let made = match ctx.globals.dry_run {
        true => {
            let roots = dc_ops::project_roots(&folder, &sources);
            let mut made = Vec::new();
            for (root, video) in roots.into_iter().zip(&sources) {
                let mut project = Project::default();
                defaults.apply(&mut project);
                let media =
                    dc_media::import_dry_run(&root.join(MEDIA_DIR), std::slice::from_ref(video))
                        .map_err(|e| CliError::Failed(e.to_string()))?;
                made.push((root, dc_ops::add_clips(&mut project, media)?));
            }
            made
        }
        false => dc_ops::new_projects(&folder, &sources, &defaults, APP)
            .map_err(|e| CliError::Failed(e.to_string()))?,
    };
    let answers: Vec<Answer> = made
        .iter()
        .map(|(root, applied)| created(root, applied, ctx.globals.dry_run))
        .collect();
    let text = answers
        .iter()
        .map(|a| a.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let json = serde_json::Value::Array(answers.into_iter().map(|a| a.json).collect());
    Ok(Answer::new(text, json))
}
