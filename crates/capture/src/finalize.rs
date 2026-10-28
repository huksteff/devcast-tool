//! Turning the segments of a recording into one ordinary MP4 per track.
//!
//! Order matters for crash safety: the track file is written under a temporary name and
//! renamed, then the manifest records it, and only then the segments are deleted. Cut off at
//! any point, the folder can be finalized again.

use std::path::{Path, PathBuf};

use dc_encode::{Mp4Layout, Mp4Reader, Mp4Writer};

use crate::error::CaptureError;
use crate::manifest::{Manifest, Track};

/// Joins the segments of every unfinished track, records the files in the manifest and
/// deletes segment and temporary files. Returns the path of each track's file.
pub fn finalize(dir: &Path, manifest: &mut Manifest) -> Result<Vec<PathBuf>, CaptureError> {
    for track in &mut manifest.tracks {
        if track.file.is_none() {
            join(dir, track)?;
            track.file = Some(track.final_file());
            track.segments.clear();
        }
    }
    manifest.write(dir)?;
    remove_leftovers(dir, manifest)?;
    Ok(manifest
        .tracks
        .iter()
        .map(|t| dir.join(t.final_file()))
        .collect())
}

fn join(dir: &Path, track: &Track) -> Result<(), CaptureError> {
    let target = dir.join(track.final_file());
    let partial = dir.join(format!("{}.tmp", track.final_file()));
    let mut writer: Option<Mp4Writer> = None;
    for segment in &track.segments {
        let mut reader = Mp4Reader::open(&dir.join(&segment.file), (1, track.rate as i32))?;
        let writer = match &mut writer {
            Some(writer) => writer,
            None => writer.insert(Mp4Writer::create(
                &partial,
                reader.parameters(),
                Mp4Layout::Plain,
            )?),
        };
        while let Some(mut packet) = reader.read()? {
            packet.shift(segment.start);
            writer.write(packet)?;
        }
    }
    let Some(writer) = writer else {
        return Err(CaptureError::EmptyRecording(dir.to_path_buf()));
    };
    writer.finish()?;
    std::fs::rename(&partial, &target).map_err(CaptureError::file("переименовать", &partial))
}

/// Deletes the files of unfinished segments and temporary files of the manifest's tracks.
/// Other files in the folder are not touched.
fn remove_leftovers(dir: &Path, manifest: &Manifest) -> Result<(), CaptureError> {
    let entries = std::fs::read_dir(dir).map_err(CaptureError::file("прочитать папку", dir))?;
    for entry in entries {
        let path = entry
            .map_err(CaptureError::file("прочитать папку", dir))?
            .path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let ours = manifest.tracks.iter().any(|t| {
            let segment = name.starts_with(&format!("{}-", t.name)) && name.ends_with(".mp4");
            let listed = t.segments.iter().any(|s| s.file == name);
            (segment && !listed) || name.ends_with(".tmp")
        });
        if ours {
            std::fs::remove_file(&path).map_err(CaptureError::file("удалить", &path))?;
        }
    }
    Ok(())
}
