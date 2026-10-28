//! The probe of a project's media file, kept in `cache/` next to the path of the media:
//! `media/recording-1/video.mp4` → `cache/media/recording-1/video.mp4.probe.json`.
//! It is valid while the file keeps its size and modification time; otherwise it is built again.
//! A cache file that does not read is built again too: `cache/` is derived data.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use dc_project::{ProjectDir, atomic};
use serde::{Deserialize, Serialize};

use crate::error::MediaError;
use crate::probe::{Probe, probe};

const SUFFIX: &str = ".probe.json";

/// What the cached probe was built from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Stamp {
    pub(crate) size: u64,
    pub(crate) modified_ns: u128,
}

#[derive(Serialize, Deserialize)]
struct Cached {
    stamp: Stamp,
    probe: Probe,
}

/// Where the cached probe of `media` (path relative to the project, with `/`) lies.
pub fn probe_cache_path(project: &ProjectDir, media: &str) -> PathBuf {
    let mut path = project.cache_dir();
    path.extend(media.split('/'));
    let mut name = path.into_os_string();
    name.push(SUFFIX);
    PathBuf::from(name)
}

/// The probe of the project's media file `media`, from the cache when the file has not changed.
pub fn cached_probe(project: &ProjectDir, media: &str) -> Result<Probe, MediaError> {
    let file = project.root().join(media);
    let cache = probe_cache_path(project, media);
    let stamp = stamp(&file)?;
    if let Some(probe) = read(&cache, stamp) {
        return Ok(probe);
    }
    let probe = probe(&file)?;
    if let Some(folder) = cache.parent() {
        std::fs::create_dir_all(folder).map_err(MediaError::io("создать папку", folder))?;
    }
    let cached = Cached { stamp, probe };
    atomic::write_json(&cache, &cached)?;
    Ok(cached.probe)
}

pub(crate) fn stamp(file: &Path) -> Result<Stamp, MediaError> {
    let metadata = std::fs::metadata(file).map_err(MediaError::io("прочитать", file))?;
    let modified = metadata
        .modified()
        .map_err(MediaError::io("узнать время изменения", file))?;
    Ok(Stamp {
        size: metadata.len(),
        modified_ns: modified
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    })
}

fn read(cache: &Path, stamp: Stamp) -> Option<Probe> {
    let text = std::fs::read_to_string(cache).ok()?;
    let cached: Cached = serde_json::from_str(&text).ok()?;
    (cached.stamp == stamp).then_some(cached.probe)
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
