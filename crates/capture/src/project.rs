//! A project from a recording. The recording folder lies in the project's `media/` as it was
//! written: the finished track files, `manifest.json` (tracks, rates, length, frame size,
//! start, pauses), `cursor.bin`, `keys.bin` and `cursor-NNNN.png`. The clip's media is the video
//! file; the sound tracks and telemetry are found next to it through the manifest and their
//! fixed names (`MediaKind::Recording`).
//!
//! A stopped recording and one recovered after its process died go into the project the same
//! way: [`add_recording`] from the finished manifest.

use std::path::{Path, PathBuf};

use dc_core::geom::PixelSize;
use dc_core::project::{Clip, ClipId, Media, MediaKind};
use dc_core::{Project, Range, Rect, SourceTime, TimeDelta};
use dc_project::dir::MEDIA_DIR;

use crate::cancel::remove_recording;
use crate::error::CaptureError;
use crate::manifest::{MANIFEST, Manifest, VIDEO_TRACK};
use crate::recover::recover;

/// Name stem of recording folders in `media/`: `recording-1`, `recording-2`, …
pub const RECORDING_FOLDER: &str = "recording";

/// The first `recording-N.dcproj` that is not in the folder `dir` yet: where a new recording
/// goes when nobody named its project.
pub fn free_project_root(dir: &Path) -> PathBuf {
    let mut n = 1;
    loop {
        let root = dir.join(format!("{RECORDING_FOLDER}-{n}.dcproj"));
        if !root.exists() {
            return root;
        }
        n += 1;
    }
}

/// The first `recording-N` that is not in the folder `media` yet.
pub fn free_recording_folder(media: &Path) -> String {
    let mut n = 1;
    loop {
        let name = format!("{RECORDING_FOLDER}-{n}");
        if !media.join(&name).exists() {
            return name;
        }
        n += 1;
    }
}

/// The folder of recording `media` in the project at `root`: the folder of its video.
pub fn recording_dir(root: &Path, media: &Media) -> PathBuf {
    let mut dir = root.join(&media.path);
    dir.pop();
    dir
}

/// Adds the finished recording in `media/<folder>/` (its `manifest`) to the end of `project`
/// as a clip of the whole video; returns the clip.
pub fn add_recording<'a>(
    project: &'a mut Project,
    folder: &str,
    manifest: &Manifest,
) -> Result<&'a Clip, CaptureError> {
    let bad = |message: &str| CaptureError::BadManifest {
        path: PathBuf::from(MEDIA_DIR).join(folder).join(MANIFEST),
        message: message.to_owned(),
    };
    let video = manifest
        .tracks
        .iter()
        .find(|t| t.name == VIDEO_TRACK)
        .ok_or_else(|| bad("нет видео-дорожки"))?;
    let file = video
        .file
        .as_ref()
        .ok_or_else(|| bad("запись не собрана"))?;
    let (width, height) = video.size.ok_or_else(|| bad("нет размера кадра"))?;
    let duration = TimeDelta::from_micros(video.length * 1_000_000 / i64::from(video.rate));
    let trim = Range::new(SourceTime::ZERO, SourceTime::ZERO + duration)
        .map_err(|_| bad("отрицательная длина"))?;
    let id = project.clip_ids.allocate();
    let index = project.clips.len();
    project.clips.push(Clip {
        id,
        media: Media {
            path: format!("{MEDIA_DIR}/{folder}/{file}"),
            kind: MediaKind::Recording,
            duration,
            size: PixelSize { width, height },
        },
        trim,
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
        audio: Default::default(),
    });
    Ok(&project.clips[index])
}

/// What became of a recording whose process died.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovered {
    /// Finished from its complete segments and added as clip `clip`.
    Clip {
        folder: String,
        clip: ClipId,
        duration: TimeDelta,
    },
    /// Broke off before its first complete segment: its files are deleted.
    Empty { folder: String },
}

/// Recordings in `media` that were never finished: a folder with a manifest that is not
/// finished. A recording still being written looks the same: call this only while holding
/// the project lock, which the recording holds until its stop.
pub fn unfinished_recordings(media: &Path) -> Result<Vec<String>, CaptureError> {
    let entries = match std::fs::read_dir(media) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(CaptureError::file("прочитать папку", media)(
                e,
            ));
        }
    };
    let mut folders = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(CaptureError::file("прочитать папку", media))?
            .path();
        if Manifest::path(&path).is_file() && !Manifest::read(&path)?.is_finished() {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            folders.extend(name);
        }
    }
    folders.sort();
    Ok(folders)
}

/// Finishes each of `folders` in `media` (as after a normal stop, from the complete segments)
/// and adds it to `project`; a recording without a complete segment is deleted.
pub fn recover_recordings(
    media: &Path,
    folders: &[String],
    project: &mut Project,
) -> Result<Vec<Recovered>, CaptureError> {
    let mut recovered = Vec::new();
    for folder in folders {
        let dir = media.join(folder);
        match recover(&dir) {
            Ok(_) => {
                let manifest = Manifest::read(&dir)?;
                let clip = add_recording(project, folder, &manifest)?;
                recovered.push(Recovered::Clip {
                    folder: folder.clone(),
                    clip: clip.id,
                    duration: clip.media.duration,
                });
            }
            Err(CaptureError::EmptyRecording(_)) => {
                remove_recording(&dir, &Manifest::read(&dir)?)?;
                recovered.push(Recovered::Empty {
                    folder: folder.clone(),
                });
            }
            Err(e) => return Err(e),
        }
    }
    Ok(recovered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::HostTime;
    use crate::manifest::Track;

    #[test]
    fn folder_names_skip_taken_ones() {
        let media = tempfile::tempdir().unwrap();
        assert_eq!(free_recording_folder(media.path()), "recording-1");
        std::fs::create_dir(media.path().join("recording-1")).unwrap();
        std::fs::create_dir(media.path().join("recording-3")).unwrap();
        assert_eq!(free_recording_folder(media.path()), "recording-2");
    }

    fn finished(rate: u32, length: i64) -> Manifest {
        let mut video = Track::new(VIDEO_TRACK, rate, HostTime::from_micros(0));
        video.file = Some(video.final_file());
        video.length = length;
        video.size = Some((1920, 1080));
        let mut sound = Track::new("system", 48_000, HostTime::from_micros(0));
        sound.file = Some(sound.final_file());
        Manifest::new(vec![sound, video])
    }

    #[test]
    fn a_finished_recording_becomes_a_clip_at_the_end() {
        let mut project = Project::default();
        let first = add_recording(&mut project, "recording-1", &finished(60, 600))
            .unwrap()
            .id;
        let second = add_recording(&mut project, "recording-2", &finished(120, 1))
            .unwrap()
            .id;
        assert_eq!(
            (first.to_string(), second.to_string()),
            ("c1".into(), "c2".into())
        );
        assert_eq!(project.problems(), Vec::<String>::new());
        let media = &project.clips[0].media;
        assert_eq!(media.path, "media/recording-1/video.mp4");
        assert_eq!(
            recording_dir(Path::new("Урок.dcproj"), media),
            Path::new("Урок.dcproj/media/recording-1")
        );
        assert_eq!(media.duration, TimeDelta::from_micros(10_000_000));
        assert_eq!((media.size.width, media.size.height), (1920, 1080));
        assert_eq!(project.clips[0].trim.duration(), media.duration);
        assert_eq!(
            project.clips[1].media.duration,
            TimeDelta::from_micros(8333)
        );
    }

    #[test]
    fn an_unfinished_manifest_is_not_a_clip() {
        let mut manifest = finished(60, 600);
        manifest.tracks[1].file = None;
        let mut project = Project::default();
        let error = add_recording(&mut project, "r", &manifest).unwrap_err();
        assert!(error.to_string().contains("не собрана"), "{error}");
        assert!(project.clips.is_empty());
    }

    #[test]
    fn only_unfinished_manifests_are_found() {
        let media = tempfile::tempdir().unwrap();
        for (folder, done) in [
            ("recording-2", false),
            ("recording-1", true),
            ("other", false),
        ] {
            let dir = media.path().join(folder);
            std::fs::create_dir(&dir).unwrap();
            let mut manifest = finished(60, 60);
            if !done {
                manifest.tracks[1].file = None;
            }
            if folder != "other" {
                manifest.write(&dir).unwrap();
            }
        }
        std::fs::write(media.path().join("a.mp4"), b"x").unwrap();
        assert_eq!(
            unfinished_recordings(media.path()).unwrap(),
            ["recording-2"]
        );
        assert!(
            unfinished_recordings(&media.path().join("нет"))
                .unwrap()
                .is_empty()
        );
    }
}
