//! Which sound tracks a clip's media has.
//!
//! - A recording (`MediaKind::Recording`) has the tracks of its `manifest.json` next to the
//!   video, except the video: `system`, `mic`. Each is a finished one-stream file.
//! - An imported video has its own audio streams, named `audio-1`, `audio-2`, … in file order.

use std::path::{Path, PathBuf};

use dc_capture::{Manifest, VIDEO_TRACK};
use dc_core::project::{Media, MediaKind};
use ffmpeg_next as ffmpeg;

use crate::error::AudioError;

/// Name prefix of the audio streams of an imported video.
pub const IMPORTED_TRACK: &str = "audio";

/// One sound track of a media file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceTrack {
    /// `system`, `mic` for a recording; `audio-N` for an imported video.
    pub name: String,
    pub path: PathBuf,
    /// Stream index inside the file.
    pub stream: usize,
}

/// Sound tracks of `media` of the project in `project_dir`, in a stable order.
pub fn tracks(project_dir: &Path, media: &Media) -> Result<Vec<SourceTrack>, AudioError> {
    let path = project_dir.join(&media.path);
    match media.kind {
        MediaKind::Recording => recording_tracks(&path),
        MediaKind::Imported => imported_tracks(&path),
    }
}

fn recording_tracks(video: &Path) -> Result<Vec<SourceTrack>, AudioError> {
    let folder = video.parent().unwrap_or(Path::new(""));
    let manifest = Manifest::read(folder)?;
    let sound = manifest.tracks.iter().filter(|t| t.name != VIDEO_TRACK);
    sound
        .map(|track| {
            let file = track
                .file
                .as_ref()
                .ok_or_else(|| AudioError::Unfinished(folder.to_path_buf()))?;
            Ok(SourceTrack {
                name: track.name.clone(),
                path: folder.join(file),
                stream: 0,
            })
        })
        .collect()
}

fn imported_tracks(path: &Path) -> Result<Vec<SourceTrack>, AudioError> {
    crate::init();
    let input = ffmpeg::format::input(path).map_err(AudioError::file("открыть", path))?;
    let audio = input
        .streams()
        .filter(|s| s.parameters().medium() == ffmpeg::media::Type::Audio);
    Ok(audio
        .enumerate()
        .map(|(n, stream)| SourceTrack {
            name: format!("{IMPORTED_TRACK}-{}", n + 1),
            path: path.to_path_buf(),
            stream: stream.index(),
        })
        .collect())
}
