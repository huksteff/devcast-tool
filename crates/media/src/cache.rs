//! The probe of a project's media file, kept in `cache/` next to the path of the media:
//! `media/recording-1/video.mp4` → `cache/media/recording-1/video.mp4.probe.json`.
//! It is valid while the file keeps its size and modification time; otherwise it is built again.
//! A cache file that does not read is built again too: `cache/` is derived data.

use std::path::{Path, PathBuf};

use dc_project::{FileStamp, ProjectDir, atomic};
use serde::{Deserialize, Serialize};

use crate::error::MediaError;
use crate::probe::{Probe, probe};

#[derive(Serialize, Deserialize)]
struct Cached {
    stamp: FileStamp,
    probe: Probe,
}

/// Where the cached probe of `media` (path relative to the project, with `/`) lies.
pub fn probe_cache_path(project: &ProjectDir, media: &str) -> PathBuf {
    project.cache_file(Path::new(media), ".probe.json")
}

/// The probe of the project's media file `media`, from the cache when the file has not changed.
pub fn cached_probe(project: &ProjectDir, media: &str) -> Result<Probe, MediaError> {
    let file = project.root().join(media);
    let cache = probe_cache_path(project, media);
    let stamp = FileStamp::of(&file)?;
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

fn read(cache: &Path, stamp: FileStamp) -> Option<Probe> {
    let text = std::fs::read_to_string(cache).ok()?;
    let cached: Cached = serde_json::from_str(&text).ok()?;
    (cached.stamp == stamp).then_some(cached.probe)
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
