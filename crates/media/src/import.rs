//! Importing videos into a project: each file is copied into `media/` under its own name (`-2`,
//! `-3` … when taken) and becomes media of kind `Imported`, without cursor telemetry. Shared by
//! `devcast new`, `devcast clip add` and the editor.

use std::path::{Path, PathBuf};

use dc_core::project::{Media, MediaKind};
use dc_project::ProjectDir;
use dc_project::dir::MEDIA_DIR;

use crate::cache::cached_probe;
use crate::error::MediaError;
use crate::probe::{Probe, probe};

/// Containers that can be imported (SCOPE, «Проекты и библиотека»).
pub const VIDEO_EXTENSIONS: [&str; 4] = ["mp4", "mov", "mkv", "webm"];

/// Copies `sources` into the project's `media/` and probes the copies (the probe goes to the
/// cache). All or nothing: on an error the copies made so far are deleted.
pub fn import(project: &ProjectDir, sources: &[PathBuf]) -> Result<Vec<Media>, MediaError> {
    let planned = plan(&project.media_dir(), sources)?;
    let mut copied = Vec::new();
    let result: Result<Vec<Media>, MediaError> = planned
        .iter()
        .map(|(source, name)| {
            let target = project.media_dir().join(name);
            std::fs::copy(source, &target).map_err(MediaError::io("скопировать", source))?;
            copied.push(target);
            let path = media_path(name);
            let probe = cached_probe(project, &path)?;
            Ok(media(path, &probe))
        })
        .collect();
    if result.is_err() {
        for file in copied {
            std::fs::remove_file(&file).map_err(MediaError::io("удалить", &file))?;
        }
    }
    result
}

/// What [`import`] would add, without copying: the sources are probed where they lie.
pub fn import_dry_run(media_dir: &Path, sources: &[PathBuf]) -> Result<Vec<Media>, MediaError> {
    plan(media_dir, sources)?
        .iter()
        .map(|(source, name)| Ok(media(media_path(name), &probe(source)?)))
        .collect()
}

fn media_path(name: &str) -> String {
    format!("{MEDIA_DIR}/{name}")
}

fn media(path: String, probe: &Probe) -> Media {
    Media {
        path,
        kind: MediaKind::Imported,
        duration: probe.duration,
        size: probe.size,
    }
}

/// The file name in `media_dir` for each source; fails before anything is copied if a source
/// is missing or not a supported container.
fn plan(media_dir: &Path, sources: &[PathBuf]) -> Result<Vec<(PathBuf, String)>, MediaError> {
    let mut names: Vec<String> = Vec::new();
    let mut planned = Vec::new();
    for source in sources {
        let extension = source
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .filter(|e| VIDEO_EXTENSIONS.contains(&e.as_str()));
        let (Some(stem), Some(extension)) = (source.file_stem(), extension) else {
            return Err(MediaError::NotAVideo {
                path: source.clone(),
            });
        };
        if !source.is_file() {
            return Err(MediaError::NotFound {
                path: source.clone(),
            });
        }
        let name = free_name(media_dir, &names, &stem.to_string_lossy(), &extension);
        names.push(name.clone());
        planned.push((source.clone(), name));
    }
    Ok(planned)
}

/// `stem.ext`, or `stem-2.ext`, `stem-3.ext` … if it is in `media_dir` or already `taken`.
fn free_name(media_dir: &Path, taken: &[String], stem: &str, extension: &str) -> String {
    let mut name = format!("{stem}.{extension}");
    let mut n = 1;
    while taken.contains(&name) || media_dir.join(&name).exists() {
        n += 1;
        name = format!("{stem}-{n}.{extension}");
    }
    name
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;
