//! Sound files of extra tracks (SCOPE «Аудио»: mp3, wav, aac, m4a, flac, ogg): their first sound
//! stream, read from its first sample, and importing one into the project.
//!
//! Frame 0 of such a track is the first decoded sample, wherever the file's clock puts it (an
//! mp3 often starts at 0.025 s after the encoder delay): a clip of the file plays it from there.

use std::path::Path;

use dc_core::TimeDelta;
use dc_project::ProjectDir;
use ffmpeg_next as ffmpeg;

use crate::error::AudioError;
use crate::reader::TrackReader;
use crate::stream::StreamDecoder;
use crate::track::SourceTrack;
use crate::{CHANNELS, RATE};

/// Sound files that can be imported as extra tracks.
pub const AUDIO_EXTENSIONS: [&str; 6] = ["mp3", "wav", "aac", "m4a", "flac", "ogg"];

/// Name of the track of a sound file, as [`SourceTrack::name`].
pub const FILE_TRACK: &str = "file";

/// Frames read at a time while finding a file's length.
const BLOCK: usize = RATE as usize;

/// The first sound stream of `path`, frame 0 at its first sample.
pub fn file_track(path: &Path) -> Result<SourceTrack, AudioError> {
    crate::init();
    let input = ffmpeg::format::input(path).map_err(AudioError::file("открыть", path))?;
    let stream = input
        .streams()
        .find(|s| s.parameters().medium() == ffmpeg::media::Type::Audio)
        .map(|s| s.index())
        .ok_or_else(|| AudioError::NoSound(path.to_path_buf()))?;
    let mut track = SourceTrack {
        name: FILE_TRACK.to_owned(),
        path: path.to_path_buf(),
        stream,
        origin: 0,
    };
    let mut decoder = StreamDecoder::open(&track)?;
    let first = decoder
        .next(&mut Vec::new())?
        .ok_or_else(|| AudioError::NoSound(path.to_path_buf()))?;
    track.origin = (first * 1e6).round() as i64;
    Ok(track)
}

/// Length of the sound of `track`, found by decoding it all (which also proves it decodes).
pub fn file_duration(track: &SourceTrack) -> Result<TimeDelta, AudioError> {
    let mut reader = TrackReader::open(track)?;
    let mut block = vec![0.0; BLOCK * CHANNELS];
    let mut frames = 0_i64;
    while let Some(read) = reader.fill_until_end(frames, &mut block)? {
        frames += read as i64;
    }
    Ok(TimeDelta::from_micros(frames * 1_000_000 / i64::from(RATE)))
}

/// A sound file put into the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSound {
    /// Path in the project with `/`, `assets/audio/music.mp3`.
    pub path: String,
    pub duration: TimeDelta,
    /// Whether this import made the file (see `dc_project::CopiedFile`).
    pub copied: bool,
}

/// Reads `source` whole and copies it into `folder` of the project (a path with `/` from the
/// project root). A file that does not decode is not copied; `dry_run` only reads it.
pub fn import_sound(
    project: &ProjectDir,
    folder: &str,
    source: &Path,
    dry_run: bool,
) -> Result<ImportedSound, AudioError> {
    let extension = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned());
    let (Some(stem), true) = (stem, AUDIO_EXTENSIONS.contains(&extension.as_str())) else {
        return Err(AudioError::NotSound(source.to_path_buf()));
    };
    if !source.is_file() {
        return Err(AudioError::NotFound(source.to_path_buf()));
    }
    let duration = file_duration(&file_track(source)?)?;
    if duration <= TimeDelta::ZERO {
        return Err(AudioError::NoSound(source.to_path_buf()));
    }
    let file = dc_project::copy_in(project, folder, source, &stem, &extension, dry_run)?;
    Ok(ImportedSound {
        path: file.path,
        duration,
        copied: file.copied,
    })
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
