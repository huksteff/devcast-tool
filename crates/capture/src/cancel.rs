//! Cancelling a recording: every file it wrote is deleted, and its folder too once it is empty.

use std::path::Path;

use crate::error::CaptureError;
use crate::input::{CURSOR_FILE, KEYS_FILE};
use crate::manifest::{MANIFEST, Manifest};

/// Deletes the files of the recording in `dir`: segments and files of the manifest's tracks,
/// temporary files, input telemetry with cursor pictures, the manifest. Files of others stay,
/// and so does the folder then.
pub fn remove_recording(dir: &Path, manifest: &Manifest) -> Result<(), CaptureError> {
    let entries = std::fs::read_dir(dir).map_err(CaptureError::file("прочитать папку", dir))?;
    for entry in entries {
        let path = entry
            .map_err(CaptureError::file("прочитать папку", dir))?
            .path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if is_recording_file(name, manifest) {
            std::fs::remove_file(&path).map_err(CaptureError::file("удалить", &path))?;
        }
    }
    let mut left = std::fs::read_dir(dir).map_err(CaptureError::file("прочитать папку", dir))?;
    if left.next().is_none() {
        std::fs::remove_dir(dir).map_err(CaptureError::file("удалить папку", dir))?;
    }
    Ok(())
}

fn is_recording_file(name: &str, manifest: &Manifest) -> bool {
    let track = manifest.tracks.iter().any(|t| {
        name == t.final_file()
            || (name.starts_with(&format!("{}-", t.name)) && name.ends_with(".mp4"))
    });
    let picture = name.starts_with("cursor-") && name.ends_with(".png");
    track || picture || name.ends_with(".tmp") || [MANIFEST, CURSOR_FILE, KEYS_FILE].contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::HostTime;
    use crate::manifest::Track;

    fn manifest() -> Manifest {
        let start = HostTime::from_micros(0);
        Manifest::new(vec![
            Track::new("video", 60, start),
            Track::new("mic", 48_000, start),
        ])
    }

    #[test]
    fn deletes_every_recording_file_and_the_empty_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("запись 1");
        std::fs::create_dir(&dir).unwrap();
        for name in [
            "manifest.json",
            "video-000001.mp4",
            "video-000002.mp4",
            "mic.mp4",
            "mic.mp4.tmp",
            "cursor.bin",
            "keys.bin",
            "cursor-0000.png",
            "cursor-0001.png",
        ] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        remove_recording(&dir, &manifest()).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn foreign_files_and_their_folder_stay() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["video.mp4", "system-000001.mp4", "заметки.txt", "logo.png"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        remove_recording(dir.path(), &manifest()).unwrap();
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left, ["logo.png", "system-000001.mp4", "заметки.txt"]);
    }
}
