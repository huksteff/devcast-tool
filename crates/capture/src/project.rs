//! A project from a recording. The recording folder lies in the project's `media/` as it was
//! written: the finished track files, `manifest.json` (tracks, rates, start, pauses),
//! `cursor.bin`, `keys.bin` and `cursor-NNNN.png`. The clip's media is the video file; the
//! sound tracks and telemetry are found next to it through the manifest and their fixed names
//! (`MediaKind::Recording`).

use std::path::Path;

use dc_core::geom::PixelSize;
use dc_core::project::{Clip, Media, MediaKind};
use dc_core::{Project, Range, Rect, SourceTime, TimeDelta};
use dc_project::Defaults;
use dc_project::dir::MEDIA_DIR;

use crate::error::CaptureError;

/// Name stem of recording folders in `media/`: `recording-1`, `recording-2`, …
pub const RECORDING_FOLDER: &str = "recording";

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

/// The media of a finished recording in `media/<folder>/`: its video file `video_file`,
/// frame `size`, length `end` ticks at `rate` per second.
pub fn recording_media(
    folder: &str,
    video_file: &str,
    size: (u32, u32),
    end: i64,
    rate: u32,
) -> Media {
    Media {
        path: format!("{MEDIA_DIR}/{folder}/{video_file}"),
        kind: MediaKind::Recording,
        duration: TimeDelta::from_micros(end * 1_000_000 / i64::from(rate)),
        size: PixelSize {
            width: size.0,
            height: size.1,
        },
    }
}

/// A new project of one clip with the whole `media`, with the app's default settings.
pub fn recording_project(media: Media, defaults: &Defaults) -> Result<Project, CaptureError> {
    let mut project = Project::default();
    defaults.apply(&mut project);
    let trim = Range::new(SourceTime::ZERO, SourceTime::ZERO + media.duration)
        .map_err(|_| CaptureError::EmptyRecording(media.path.clone().into()))?;
    let id = project.clip_ids.allocate();
    project.clips.push(Clip {
        id,
        media,
        trim,
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
    });
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names_skip_taken_ones() {
        let media = tempfile::tempdir().unwrap();
        assert_eq!(free_recording_folder(media.path()), "recording-1");
        std::fs::create_dir(media.path().join("recording-1")).unwrap();
        std::fs::create_dir(media.path().join("recording-3")).unwrap();
        assert_eq!(free_recording_folder(media.path()), "recording-2");
    }

    #[test]
    fn project_has_one_clip_of_the_whole_recording_and_the_defaults() {
        let media = recording_media("recording-1", "video.mp4", (1920, 1080), 600, 60);
        assert_eq!(media.path, "media/recording-1/video.mp4");
        assert_eq!(media.duration, TimeDelta::from_micros(10_000_000));
        let mut styled = Project::default();
        styled.effects.insert(
            "zoom".to_owned(),
            serde_json::json!({"settings": {"smoothness": 0.7}}),
        );
        let defaults = Defaults::from_project(&styled);
        let project = recording_project(media.clone(), &defaults).unwrap();
        assert_eq!(project.problems(), Vec::<String>::new());
        assert_eq!(project.effects, styled.effects);
        let [clip] = &project.clips[..] else {
            panic!("{:?}", project.clips);
        };
        assert_eq!(clip.id.to_string(), "c1");
        assert_eq!(clip.media, media);
        assert_eq!(clip.trim.duration(), media.duration);
        assert_eq!(clip.media.size.width, 1920);
    }

    #[test]
    fn a_120_fps_length_is_exact_to_the_microsecond() {
        let media = recording_media("r", "video.mp4", (640, 360), 1, 120);
        assert_eq!(media.duration, TimeDelta::from_micros(8333));
    }
}
