//! A whole recording: video and sound tracks into one folder with one manifest, all on the
//! host clock (decision 008). Every track starts at the first video frame and ends at the stop;
//! at the stop each track is joined into its own MP4.

use std::path::{Path, PathBuf};

use super::clock::now;
use super::video::{VideoRecording, VideoStart, VideoSummary};
use crate::audio::{AudioRecording, AudioSource, AudioSummary, MIC_TRACK, SYSTEM_TRACK};
use crate::config::RecordingConfig;
use crate::error::CaptureError;
use crate::finalize::finalize;
use crate::manifest::SharedManifest;

pub struct Recording {
    manifest: SharedManifest,
    video: VideoRecording,
    audio: Vec<AudioRecording>,
}

/// What was written.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordingSummary {
    pub video: VideoSummary,
    pub audio: Vec<AudioSummary>,
    /// The finished MP4 of each track, in manifest order (video first).
    pub files: Vec<PathBuf>,
}

impl Recording {
    /// Starts recording into the folder `dir` (created if missing; must not hold another
    /// recording).
    pub fn start(config: &RecordingConfig, dir: &Path) -> Result<Self, CaptureError> {
        config.validate()?;
        let manifest = SharedManifest::create(dir)?;
        // Sound streams open first, so the beginning is not lost while the codec is checked.
        let mut audio = Vec::new();
        if config.system_audio {
            audio.push(AudioRecording::start(AudioSource::System, SYSTEM_TRACK)?);
        }
        if let Some(name) = &config.microphone {
            let source = AudioSource::Microphone(name.clone());
            audio.push(AudioRecording::start(source, MIC_TRACK)?);
        }
        let video = VideoRecording::start(&config.source, config.fps, &manifest)?;
        let start = video.info().start;
        for track in &audio {
            track.begin(start, &manifest)?;
        }
        Ok(Self {
            manifest,
            video,
            audio,
        })
    }

    pub fn info(&self) -> VideoStart {
        self.video.info()
    }

    /// Stops every track now and joins each into its MP4.
    pub fn stop(self) -> Result<RecordingSummary, CaptureError> {
        let at = now()?;
        for track in &self.audio {
            track.stop(at)?;
        }
        let video = self.video.stop(at)?;
        let audio = self
            .audio
            .into_iter()
            .map(AudioRecording::join)
            .collect::<Result<Vec<_>, _>>()?;
        let mut manifest = self.manifest.get()?;
        let files = finalize(self.manifest.dir(), &mut manifest)?;
        Ok(RecordingSummary {
            video,
            audio,
            files,
        })
    }
}
