//! The recordings of the library: the projects `*.devcast` of the recordings folder (recordings
//! and imported videos alike, `dc_ops::new_project`), newest first by their `project.json`.
//! A project that cannot be read (a recording still going on, a broken file) is left out with a
//! warning in the log.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use dc_core::project::{Media, MediaKind};
use dc_core::{PixelSize, TimeDelta};
use dc_project::{ProjectDir, ProjectError};

/// A recording of the library.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub root: PathBuf,
    /// The folder without `.devcast`.
    pub name: String,
    /// What `clip add` of it adds: its media, each once.
    pub duration: TimeDelta,
    /// Frame of its first clip.
    pub size: PixelSize,
    /// Only imported videos, no recording.
    pub imported: bool,
    modified: SystemTime,
}

/// The recordings in `folder` except the project `open` (the editor's own).
pub fn scan(folder: &Path, open: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut entries: Vec<Entry> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.extension().is_some_and(|e| e == "devcast") && p != open)
        .filter_map(|root| match entry(&root) {
            Ok(entry) => entry,
            Err(error) => {
                tracing::warn!("библиотека пропускает {}: {error}", root.display());
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.name.cmp(&b.name)));
    entries
}

/// The entry of the project in `root`; none without clips.
fn entry(root: &Path) -> Result<Option<Entry>, ProjectError> {
    let dir = ProjectDir::open(root)?;
    let project = dir.load()?;
    let mut media: Vec<&Media> = Vec::new();
    for clip in &project.clips {
        if !media.iter().any(|m| m.path == clip.media.path) {
            media.push(&clip.media);
        }
    }
    let Some(first) = media.first() else {
        return Ok(None);
    };
    let file = dir.file(dc_project::dir::PROJECT_FILE);
    let modified = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .map_err(|source| ProjectError::Io {
            action: "прочитать",
            path: file,
            source,
        })?;
    let name = root
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Some(Entry {
        root: root.to_path_buf(),
        name,
        duration: TimeDelta::from_micros(media.iter().map(|m| m.duration.micros()).sum()),
        size: first.size,
        imported: media.iter().all(|m| m.kind == MediaKind::Imported),
        modified,
    }))
}
