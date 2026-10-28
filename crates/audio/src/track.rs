//! Which sound tracks a clip's media has.
//!
//! - A recording (`MediaKind::Recording`) has the tracks of its `manifest.json` next to the
//!   video, except the video: `system`, `mic`. Each is a finished one-stream file.
//! - An imported video has its own audio streams, named `audio-1`, `audio-2`, … in file order.
//!
//! Every track counts time from the media's first video frame (`dc_media::Probe::origin_micros`,
//! from the probe cache), so sound and picture keep the offset they have in the file. A recording's
//! tracks share tick 0 with its video (decision 008).

use std::path::{Path, PathBuf};

use dc_capture::{Manifest, VIDEO_TRACK};
use dc_core::project::{Media, MediaKind};
use dc_project::ProjectDir;
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
    /// Stream time of source time zero, microseconds: the media's first video frame.
    pub origin: i64,
}

/// Sound tracks of `media` of `project`, in a stable order.
pub fn tracks(project: &ProjectDir, media: &Media) -> Result<Vec<SourceTrack>, AudioError> {
    let path = project.root().join(&media.path);
    let origin = dc_media::cached_probe(project, &media.path)?.origin_micros;
    match media.kind {
        MediaKind::Recording => recording_tracks(&path, origin),
        MediaKind::Imported => imported_tracks(&path, origin),
    }
}

fn recording_tracks(video: &Path, origin: i64) -> Result<Vec<SourceTrack>, AudioError> {
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
                origin,
            })
        })
        .collect()
}

fn imported_tracks(path: &Path, origin: i64) -> Result<Vec<SourceTrack>, AudioError> {
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
            origin,
        })
        .collect())
}
