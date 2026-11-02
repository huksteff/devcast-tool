//! A new project from videos, one way for `devcast new` and «Импорт…» of the editor's library:
//! the folder is created, the videos are copied into its `media/` (`dc_media::import`) and become
//! clips (`clip add`) of a project with `defaults`; its history starts empty. If anything fails,
//! the new folder is deleted.

use std::path::{Path, PathBuf};

use dc_core::project::Media;
use dc_core::{History, Project};
use dc_fx_cut::ClipAdd;
use dc_media::MediaError;
use dc_project::{Defaults, ProjectDir, ProjectError, ProjectLock};

use crate::{Applied, OpsError, apply_commands, as_commands};

#[derive(Debug, thiserror::Error)]
pub enum NewProjectError {
    #[error(transparent)]
    Media(#[from] MediaError),
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Ops(#[from] OpsError),
    #[error("{error}; папку {} удалить не удалось ({source}): удали её вручную", .root.display())]
    Leftover {
        root: PathBuf,
        error: Box<NewProjectError>,
        source: std::io::Error,
    },
}

/// Creates the project `root` (`name.devcast`, not there yet) of `videos`; `app` names the
/// writer in its lock meanwhile.
pub fn new_project(
    root: &Path,
    videos: &[PathBuf],
    defaults: &Defaults,
    app: &str,
) -> Result<Applied, NewProjectError> {
    let dir = ProjectDir::create(root)?;
    let lock = ProjectLock::acquire(&dir, app)?;
    let filled = fill(&dir, videos, defaults);
    drop(lock);
    filled.map_err(|error| match std::fs::remove_dir_all(root) {
        Ok(()) => error,
        Err(source) => NewProjectError::Leftover {
            root: root.to_path_buf(),
            error: Box::new(error),
            source,
        },
    })
}

fn fill(
    dir: &ProjectDir,
    videos: &[PathBuf],
    defaults: &Defaults,
) -> Result<Applied, NewProjectError> {
    let mut project = Project::default();
    defaults.apply(&mut project);
    let media = dc_media::import(dir, videos)?;
    let applied = add_clips(&mut project, media)?;
    dir.save(&project)?;
    Ok(applied)
}

/// Adds a clip per media to a new `project` (its history starts empty).
pub fn add_clips(project: &mut Project, media: Vec<Media>) -> Result<Applied, OpsError> {
    let commands: Vec<ClipAdd> = media.into_iter().map(|media| ClipAdd { media }).collect();
    apply_commands(
        project,
        &mut History::default(),
        ClipAdd::NAME,
        &as_commands(&commands),
        false,
    )
}
