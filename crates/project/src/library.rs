//! The projects `*.devcast` of a folder with recordings (the recordings folder of the app): the
//! editor's library and `devcast ls projects` read the same list, newest first by their
//! `project.json`. A project without clips is not listed; one that cannot be read (a recording
//! still going on, a broken file) is returned apart with why.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use dc_core::project::{Media, MediaKind};
use dc_core::{PixelSize, TimeDelta};

use crate::dir::PROJECT_FILE;
use crate::{ProjectDir, ProjectError};

/// A project of the folder.
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
    /// When its `project.json` was written.
    pub modified: SystemTime,
}

/// The projects of a folder and those that could not be read.
#[derive(Debug, Default)]
pub struct Scan {
    pub entries: Vec<Entry>,
    pub skipped: Vec<(PathBuf, ProjectError)>,
}

/// The projects in `folder`; no folder — none.
pub fn scan(folder: &Path) -> Scan {
    let Ok(read) = std::fs::read_dir(folder) else {
        return Scan::default();
    };
    let mut scan = Scan::default();
    let roots = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.extension().is_some_and(|e| e == "devcast"));
    for root in roots {
        match entry(&root) {
            Ok(Some(entry)) => scan.entries.push(entry),
            Ok(None) => {}
            Err(error) => scan.skipped.push((root, error)),
        }
    }
    scan.entries
        .sort_by(|a, b| b.modified.cmp(&a.modified).then(a.name.cmp(&b.name)));
    scan
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
    let file = dir.file(PROJECT_FILE);
    let modified = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .map_err(|source| ProjectError::Io {
            action: "прочитать",
            path: file,
            source,
        })?;
    Ok(Some(Entry {
        root: root.to_path_buf(),
        name: root
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        duration: TimeDelta::from_micros(media.iter().map(|m| m.duration.micros()).sum()),
        size: first.size,
        imported: media.iter().all(|m| m.kind == MediaKind::Imported),
        modified,
    }))
}
