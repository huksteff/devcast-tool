//! `devcast new <video...> -o name.devcast` and `devcast clip add <video...>`: videos are copied
//! into the project's `media/` (`dc_media::import`) and become clips at the end of the main track
//! through the command `clip add` (`dc_fx_cut::ClipAdd`).

use std::path::{Path, PathBuf};

use dc_core::project::Media;
use dc_core::{History, Project};
use dc_fx_cut::ClipAdd;
use dc_ops::{Applied, as_commands};
use dc_project::dir::MEDIA_DIR;
use dc_project::{Defaults, ProjectDir, ProjectLock};

use crate::edit::{self, Ctx, applied_json};
use crate::error::CliError;
use crate::find::new_folder;
use crate::run::Answer;

/// `devcast clip add <video...>`: one undo step for all videos.
pub(crate) fn clip_add(ctx: &Ctx<'_>, videos: &[PathBuf]) -> Result<Answer, CliError> {
    let sources = sources(ctx.cwd, videos);
    let mut workspace = ctx.open(true)?;
    let media = match ctx.globals.dry_run {
        true => dc_media::import_dry_run(&workspace.dir().media_dir(), &sources),
        false => dc_media::import(workspace.dir(), &sources),
    }
    .map_err(failed)?;
    let commands = clip_adds(media);
    edit::apply_commands(ctx, &mut workspace, ClipAdd::NAME, &as_commands(&commands))
}

/// `devcast new <video...> -o name.devcast`: a project with the app defaults and a clip per video.
/// If an import fails, the new folder is deleted.
pub(crate) fn new(ctx: &Ctx<'_>, videos: &[PathBuf], out: &Path) -> Result<Answer, CliError> {
    let root = new_folder(ctx.cwd, out)?;
    let sources = sources(ctx.cwd, videos);
    let mut project = Project::default();
    Defaults::load(&ctx.defaults_file()?)?.apply(&mut project);
    if ctx.globals.dry_run {
        let media = dc_media::import_dry_run(&root.join(MEDIA_DIR), &sources).map_err(failed)?;
        let applied = add_clips(&mut project, media)?;
        return Ok(created(&root, &applied, true));
    }
    let dir = ProjectDir::create(&root)?;
    let lock = ProjectLock::acquire(&dir, "devcast new")?;
    let filled = dc_media::import(&dir, &sources)
        .map_err(failed)
        .and_then(|media| add_clips(&mut project, media))
        .and_then(|applied| Ok(dir.save(&project).map(|()| applied)?));
    drop(lock);
    match filled {
        Ok(applied) => Ok(created(&root, &applied, false)),
        Err(error) => {
            std::fs::remove_dir_all(&root).map_err(|e| {
                CliError::Failed(format!(
                    "{error}; папку {} удалить не удалось ({e}): удали её вручную",
                    root.display()
                ))
            })?;
            Err(error)
        }
    }
}

fn sources(cwd: &Path, videos: &[PathBuf]) -> Vec<PathBuf> {
    videos.iter().map(|video| cwd.join(video)).collect()
}

fn failed(error: dc_media::MediaError) -> CliError {
    CliError::Failed(error.to_string())
}

fn clip_adds(media: Vec<Media>) -> Vec<ClipAdd> {
    media.into_iter().map(|media| ClipAdd { media }).collect()
}

/// The clips of a new project; its history starts empty.
fn add_clips(project: &mut Project, media: Vec<Media>) -> Result<Applied, CliError> {
    let commands = clip_adds(media);
    let applied = dc_ops::apply_commands(
        project,
        &mut History::default(),
        ClipAdd::NAME,
        &as_commands(&commands),
        false,
    )?;
    Ok(applied)
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
