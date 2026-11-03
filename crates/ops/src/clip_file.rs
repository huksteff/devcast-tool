//! `clip add` of videos and of DevCast projects, one way for `devcast clip add` and the editor's
//! library: a video file is copied into `media/` (`dc_media::import`, kind `Imported`); a project
//! folder gives the media of its clips, each once in clip order: a recording's whole folder
//! (`media/recording-N/`: video, sound, cursor, keys) is copied into a free `recording-M` and stays
//! a `Recording`, an imported video is copied as a video. The caller applies the commands as one
//! undo step; when they are refused, `discard` deletes the copies this import made.

use std::path::{Path, PathBuf};

use dc_capture::project::{RECORDING_FOLDER, recording_dir};
use dc_core::project::{Media, MediaKind};
use dc_fx_cut::ClipAdd;
use dc_media::MediaError;
use dc_project::dir::MEDIA_DIR;
use dc_project::{ProjectDir, ProjectError};

#[derive(Debug, thiserror::Error)]
pub enum ClipImportError {
    #[error(transparent)]
    Media(#[from] MediaError),
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error("в проекте «{}» нет клипов: добавлять нечего, выбери другую запись", .0.display())]
    Empty(PathBuf),
    #[error("не удалось скопировать «{}» в «{}» ({source}): проверь место на диске и попробуй снова", .from.display(), .to.display())]
    Copy {
        from: PathBuf,
        to: PathBuf,
        source: std::io::Error,
    },
    #[error("копию «{}» удалить не удалось ({}): удали её вручную", .0.display(), .1)]
    Discard(PathBuf, #[source] std::io::Error),
}

/// The clips to add and the copies made for them.
#[derive(Debug)]
pub struct ClipImport {
    pub commands: Vec<ClipAdd>,
    /// Files and recording folders this import made: deleted when the commands are refused.
    copies: Vec<PathBuf>,
}

/// Copies `sources` (videos and project folders) into the project in `dir` (not with
/// `dry_run`) and makes a `clip add` per media, in order. All or nothing: on an error the copies
/// made so far are deleted.
pub fn import_clips(
    dir: &ProjectDir,
    sources: &[PathBuf],
    dry_run: bool,
) -> Result<ClipImport, ClipImportError> {
    let mut import = ClipImport {
        commands: Vec::new(),
        copies: Vec::new(),
    };
    let mut folders = Vec::new();
    for source in sources {
        let added = match source.is_dir() {
            true => add_project(dir, source, dry_run, &mut import, &mut folders),
            false => add_video(dir, source, dry_run, &mut import),
        };
        if let Err(error) = added {
            import.discard()?;
            return Err(error);
        }
    }
    Ok(import)
}

impl ClipImport {
    /// The name of its commands in the history.
    pub const NAME: &'static str = ClipAdd::NAME;

    /// Deletes the copies this import made: the commands were refused.
    pub fn discard(&self) -> Result<(), ClipImportError> {
        for copy in &self.copies {
            let removed = match copy.is_dir() {
                true => std::fs::remove_dir_all(copy),
                false => std::fs::remove_file(copy),
            };
            removed.map_err(|e| ClipImportError::Discard(copy.clone(), e))?;
        }
        Ok(())
    }
}

fn add_video(
    dir: &ProjectDir,
    video: &Path,
    dry_run: bool,
    import: &mut ClipImport,
) -> Result<(), ClipImportError> {
    let sources = [video.to_path_buf()];
    let media = match dry_run {
        true => dc_media::import_dry_run(&dir.media_dir(), &sources)?,
        false => dc_media::import(dir, &sources)?,
    };
    for media in media {
        if !dry_run {
            import.copies.push(dir.root().join(&media.path));
        }
        import.commands.push(ClipAdd { media });
    }
    Ok(())
}

/// The media of the clips of the project in `root`; `folders` are the recording folders taken
/// by this import so far.
fn add_project(
    dir: &ProjectDir,
    root: &Path,
    dry_run: bool,
    import: &mut ClipImport,
    folders: &mut Vec<String>,
) -> Result<(), ClipImportError> {
    let source = ProjectDir::open(root)?;
    let project = source.load()?;
    let mut media: Vec<&Media> = Vec::new();
    for clip in &project.clips {
        if !media.iter().any(|m| m.path == clip.media.path) {
            media.push(&clip.media);
        }
    }
    if media.is_empty() {
        return Err(ClipImportError::Empty(root.to_path_buf()));
    }
    for media in media {
        match media.kind {
            MediaKind::Imported => add_video(dir, &root.join(&media.path), dry_run, import)?,
            MediaKind::Recording => {
                let copied = copy_recording(dir, root, media, dry_run, folders)?;
                if !dry_run {
                    import.copies.push(recording_dir(dir.root(), &copied));
                }
                import.commands.push(ClipAdd { media: copied });
            }
        }
    }
    Ok(())
}

/// Copies the folder of recording `media` of the project in `root` into a free `recording-M`
/// of `dir`; the media of the copy.
fn copy_recording(
    dir: &ProjectDir,
    root: &Path,
    media: &Media,
    dry_run: bool,
    folders: &mut Vec<String>,
) -> Result<Media, ClipImportError> {
    let from = recording_dir(root, media);
    let file = Path::new(&media.path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let target = dir.media_dir();
    let folder = free_folder(&target, folders);
    folders.push(folder.clone());
    if !dry_run {
        let to = target.join(&folder);
        copy_dir(&from, &to).map_err(|source| ClipImportError::Copy {
            from: from.clone(),
            to: to.clone(),
            source,
        })?;
    }
    Ok(Media {
        path: format!("{MEDIA_DIR}/{folder}/{file}"),
        ..media.clone()
    })
}

/// The first `recording-N` neither in `media` nor `taken`.
fn free_folder(media: &Path, taken: &[String]) -> String {
    (1..)
        .map(|n| format!("{RECORDING_FOLDER}-{n}"))
        .find(|name| !taken.contains(name) && !media.join(name).exists())
        .unwrap_or_default()
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        match entry.file_type()?.is_dir() {
            true => copy_dir(&entry.path(), &target)?,
            false => std::fs::copy(entry.path(), &target).map(drop)?,
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "clip_file_tests.rs"]
mod tests;
