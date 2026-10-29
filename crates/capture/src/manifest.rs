//! `manifest.json` of a recording folder: which segment files of each track are complete.
//!
//! The manifest is rewritten atomically after every finished segment, so after a crash it
//! lists exactly the segments that can be trusted. A file the manifest does not list is an
//! unfinished segment or a temporary file.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::error::CaptureError;
use crate::event::HostTime;
use crate::pause::{Pause, Pauses};

pub const MANIFEST: &str = "manifest.json";

/// Name of the video track in the manifest and of its files.
pub const VIDEO_TRACK: &str = "video";

/// Version of the manifest format; a newer one is refused.
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub tracks: Vec<Track>,
    /// Pauses on the host clock. They are cut out of every track: tick `n` of a track is at
    /// `start + n / rate` on the recording clock, which is the host clock minus the pauses
    /// before that moment (see [`crate::pause`]).
    pub pauses: Vec<Pause>,
}

/// One recorded stream: video, system sound or microphone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    /// Track name, also the stem of its files: `video` → `video-000001.mp4`, `video.mp4`.
    pub name: String,
    /// Ticks per second of `start`/`end`: the frame rate for video, the sample rate for sound.
    pub rate: u32,
    /// Host time of tick 0. All tracks of a recording share the host clock (QPC), so this is
    /// what lines them up.
    pub start: HostTime,
    /// Complete segments in order; together they cover the track without gaps.
    pub segments: Vec<Segment>,
    /// The finished track file; once it is written the segments are deleted.
    pub file: Option<String>,
    /// Length of the finished file in ticks, kept when the segments are deleted.
    pub length: i64,
    /// Frame size of a video track (width, height); `None` for sound.
    pub size: Option<(u32, u32)>,
}

/// A finished segment: its packets have times from 0, at `start` ticks into the track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub file: String,
    pub start: i64,
    /// Exclusive: the next segment starts here.
    pub end: i64,
}

impl Track {
    pub fn new(name: &str, rate: u32, start: HostTime) -> Self {
        Self {
            name: name.to_owned(),
            rate,
            start,
            segments: Vec::new(),
            file: None,
            length: 0,
            size: None,
        }
    }

    /// Where the track ends, in ticks: the last complete segment, or the finished file.
    pub fn end(&self) -> i64 {
        self.segments.last().map_or(self.length, |s| s.end)
    }

    /// File name of segment `index` (from 1).
    pub fn segment_file(&self, index: usize) -> String {
        format!("{}-{index:06}.mp4", self.name)
    }

    /// File name of the finished track.
    pub fn final_file(&self) -> String {
        format!("{}.mp4", self.name)
    }
}

impl Manifest {
    pub fn new(tracks: Vec<Track>) -> Self {
        Self {
            version: VERSION,
            tracks,
            pauses: Vec::new(),
        }
    }

    /// Every track has its finished file.
    pub fn is_finished(&self) -> bool {
        self.tracks.iter().all(|t| t.file.is_some())
    }

    pub fn path(dir: &Path) -> PathBuf {
        dir.join(MANIFEST)
    }

    pub fn read(dir: &Path) -> Result<Self, CaptureError> {
        let path = Self::path(dir);
        let bad = |message: String| CaptureError::BadManifest {
            path: path.clone(),
            message,
        };
        let text = std::fs::read_to_string(&path).map_err(|e| bad(e.to_string()))?;
        let manifest: Self = serde_json::from_str(&text).map_err(|e| bad(e.to_string()))?;
        if manifest.version > VERSION {
            return Err(bad(format!(
                "версия {} новее, чем умеет программа ({VERSION}): обнови программу",
                manifest.version
            )));
        }
        Ok(manifest)
    }

    /// Replaces the file atomically: a crash leaves the old manifest or the new one.
    pub fn write(&self, dir: &Path) -> Result<(), CaptureError> {
        Ok(dc_project::atomic::write_json(&Self::path(dir), self)?)
    }
}

/// The manifest of a recording being written, shared by the threads writing its tracks: each
/// changes only its own track, and every change rewrites the file atomically.
#[derive(Clone)]
pub struct SharedManifest {
    dir: PathBuf,
    manifest: Arc<Mutex<Manifest>>,
}

impl SharedManifest {
    /// Starts an empty manifest in `dir` (created if missing; must not hold another recording).
    pub fn create(dir: &Path) -> Result<Self, CaptureError> {
        std::fs::create_dir_all(dir).map_err(CaptureError::file("создать папку", dir))?;
        if Manifest::path(dir).exists() {
            return Err(CaptureError::RecordingExists(dir.to_path_buf()));
        }
        let manifest = Manifest::new(Vec::new());
        manifest.write(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
            manifest: Arc::new(Mutex::new(manifest)),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Adds `track` or replaces the track of the same name, and writes the file.
    pub fn set(&self, track: &Track) -> Result<(), CaptureError> {
        let mut manifest = self.lock()?;
        match manifest.tracks.iter_mut().find(|t| t.name == track.name) {
            Some(old) => old.clone_from(track),
            None => manifest.tracks.push(track.clone()),
        }
        manifest.write(&self.dir)
    }

    /// Records the pauses so far and writes the file.
    pub fn set_pauses(&self, pauses: &Pauses) -> Result<(), CaptureError> {
        let mut manifest = self.lock()?;
        pauses.list().clone_into(&mut manifest.pauses);
        manifest.write(&self.dir)
    }

    pub fn get(&self) -> Result<Manifest, CaptureError> {
        Ok(self.lock()?.clone())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Manifest>, CaptureError> {
        self.manifest.lock().map_err(|_| {
            CaptureError::platform("записать manifest.json")(
                "поток записи дорожки упал".to_owned(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trips_and_names_files() {
        let mut track = Track::new("video", 60, HostTime::from_micros(5_000_000));
        track.segments.push(Segment {
            file: track.segment_file(1),
            start: 0,
            end: 120,
        });
        assert_eq!(track.segments[0].file, "video-000001.mp4");
        assert_eq!(track.final_file(), "video.mp4");
        assert_eq!(track.end(), 120);
        let manifest = Manifest::new(vec![track]);
        assert!(!manifest.is_finished());
        let json = serde_json::to_string(&manifest).unwrap();
        assert_eq!(
            json,
            r#"{"version":1,"tracks":[{"name":"video","rate":60,"start":5000000,"segments":[{"file":"video-000001.mp4","start":0,"end":120}],"file":null,"length":0,"size":null}],"pauses":[]}"#
        );
        assert_eq!(serde_json::from_str::<Manifest>(&json).unwrap(), manifest);
    }

    #[test]
    fn shared_manifest_keeps_every_track() {
        let dir = tempfile::tempdir().unwrap();
        let shared = SharedManifest::create(dir.path()).unwrap();
        assert!(matches!(
            SharedManifest::create(dir.path()),
            Err(CaptureError::RecordingExists(_))
        ));
        let mut video = Track::new("video", 60, HostTime::from_micros(10));
        let system = Track::new("system", 48_000, HostTime::from_micros(10));
        shared.set(&video).unwrap();
        shared.set(&system).unwrap();
        video.segments.push(Segment {
            file: video.segment_file(1),
            start: 0,
            end: 120,
        });
        shared.set(&video).unwrap();
        let read = Manifest::read(dir.path()).unwrap();
        assert_eq!(read, Manifest::new(vec![video, system]));
        assert_eq!(shared.get().unwrap(), read);
        let mut pauses = Pauses::default();
        pauses.pause(HostTime::from_micros(100)).unwrap();
        shared.set_pauses(&pauses).unwrap();
        let read = Manifest::read(dir.path()).unwrap();
        assert_eq!(read.pauses, pauses.list());
        assert_eq!(read.tracks.len(), 2);
    }

    #[test]
    fn newer_or_broken_manifest_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut manifest = Manifest::new(vec![]);
        manifest.version = VERSION + 1;
        manifest.write(dir.path()).unwrap();
        let error = Manifest::read(dir.path()).unwrap_err();
        assert!(error.to_string().contains("обнови программу"), "{error}");
        std::fs::write(Manifest::path(dir.path()), "{\"version\":1,").unwrap();
        assert!(matches!(
            Manifest::read(dir.path()),
            Err(CaptureError::BadManifest { .. })
        ));
    }
}
