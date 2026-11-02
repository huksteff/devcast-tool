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

/// The folders of a project per video in `folder` (`devcast new --each`, «Импорт…» of the
/// library): `<video name>.devcast`, then `-2`, `-3` … when the folder is there or another video
/// of `videos` took the name.
pub fn project_roots(folder: &Path, videos: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for video in videos {
        let stem = video.file_stem().unwrap_or_default().to_string_lossy();
        let root = (1..)
            .map(|n| match n {
                1 => folder.join(format!("{stem}.devcast")),
                n => folder.join(format!("{stem}-{n}.devcast")),
            })
            .find(|root| !root.exists() && !roots.contains(root))
            .unwrap_or_default();
        roots.push(root);
    }
    roots
}

/// A project of each of `videos` in `folder` (created when missing) at [`project_roots`], with
/// what each got. A video that fails stops the rest; the projects made before it stay.
pub fn new_projects(
    folder: &Path,
    videos: &[PathBuf],
    defaults: &Defaults,
    app: &str,
) -> Result<Vec<(PathBuf, Applied)>, NewProjectError> {
    std::fs::create_dir_all(folder).map_err(|source| ProjectError::Io {
        action: "создать папку",
        path: folder.to_path_buf(),
        source,
    })?;
    let roots = project_roots(folder, videos);
    let mut made = Vec::new();
    for (root, video) in roots.into_iter().zip(videos) {
        let applied = new_project(&root, std::slice::from_ref(video), defaults, app)?;
        made.push((root, applied));
    }
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_video_name_taken_on_disk_or_by_another_video_gets_a_number() {
        let tmp = tempfile::tempdir().unwrap();
        let folder = tmp.path().join("Мои записи");
        std::fs::create_dir_all(folder.join("демо урок.devcast")).unwrap();
        let videos = [
            PathBuf::from("a/демо урок.mp4"),
            PathBuf::from("b/демо урок.mov"),
            PathBuf::from("вступление.mkv"),
        ];
        let roots = project_roots(&folder, &videos);
        assert_eq!(
            roots,
            [
                folder.join("демо урок-2.devcast"),
                folder.join("демо урок-3.devcast"),
                folder.join("вступление.devcast"),
            ]
        );
    }
}
