//! `manifest.json` of a recording folder: which segment files of each track are complete.
//!
//! The manifest is rewritten atomically after every finished segment, so after a crash it
//! lists exactly the segments that can be trusted. A file the manifest does not list is an
//! unfinished segment or a temporary file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::CaptureError;
use crate::event::HostTime;

pub const MANIFEST: &str = "manifest.json";

/// Version of the manifest format; a newer one is refused.
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub tracks: Vec<Track>,
}

/// One recorded stream (video now; system sound and microphone later).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    /// Track name, also the stem of its files: `video` → `video-000001.mp4`, `video.mp4`.
    pub name: String,
    /// Ticks per second of `start`/`end` (the frame rate for video).
    pub rate: u32,
    /// Host time of tick 0.
    pub start: HostTime,
    /// Complete segments in order; together they cover the track without gaps.
    pub segments: Vec<Segment>,
    /// The finished track file; once it is written the segments are deleted.
    pub file: Option<String>,
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
        }
    }

    /// Where the track ends, in ticks.
    pub fn end(&self) -> i64 {
        self.segments.last().map_or(0, |s| s.end)
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
            r#"{"version":1,"tracks":[{"name":"video","rate":60,"start":5000000,"segments":[{"file":"video-000001.mp4","start":0,"end":120}],"file":null}]}"#
        );
        assert_eq!(serde_json::from_str::<Manifest>(&json).unwrap(), manifest);
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
