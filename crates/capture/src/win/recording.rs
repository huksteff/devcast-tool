//! A whole recording: video, sound and input into one folder with one manifest, all on the
//! host clock (decisions 008, 009). Every track starts at the first video frame and ends at
//! the stop; pauses are cut out of every track ([`crate::pause`]). At the stop each track is
//! joined into its own MP4; a cancel deletes every file.

use std::path::{Path, PathBuf};

use super::clock::now;
use super::input::InputRecording;
use super::origin::source_origin;
use super::video::{VideoRecording, VideoStart, VideoSummary};
use crate::audio::{AudioRecording, AudioSource, AudioSummary, MIC_TRACK, SYSTEM_TRACK};
use crate::cancel::remove_recording;
use crate::config::RecordingConfig;
use crate::error::CaptureError;
use crate::event::HostTime;
use crate::finalize::finalize;
use crate::input::InputSummary;
use crate::manifest::{Manifest, SharedManifest};
use crate::pause::PauseClock;

pub struct Recording {
    manifest: SharedManifest,
    clock: PauseClock,
    video: VideoRecording,
    audio: Vec<AudioRecording>,
    input: InputRecording,
}

/// What was written.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordingSummary {
    pub video: VideoSummary,
    pub audio: Vec<AudioSummary>,
    /// The input written, or why it stopped early: the recording is kept without it.
    pub input: Result<InputSummary, String>,
    /// The finished MP4 of each track, in manifest order (video first).
    pub files: Vec<PathBuf>,
    /// The manifest of the finished recording: tracks with their files, pauses.
    pub manifest: Manifest,
}

impl RecordingSummary {
    /// What went wrong without losing the recording: video, each sound track, input.
    pub fn problems(&self) -> Vec<String> {
        let sound = self.audio.iter().flat_map(|a| a.problems.iter().cloned());
        let input = self.input.as_ref().err().cloned();
        self.video
            .problems
            .iter()
            .cloned()
            .chain(sound)
            .chain(input)
            .collect()
    }
}

/// The tracks once they are stopped.
struct Stopped {
    video: VideoSummary,
    audio: Vec<AudioSummary>,
    input: Result<InputSummary, String>,
}

impl Recording {
    /// Starts recording into the folder `dir` (created if missing; must not hold another
    /// recording).
    pub fn start(config: &RecordingConfig, dir: &Path) -> Result<Self, CaptureError> {
        config.validate()?;
        let manifest = SharedManifest::create(dir)?;
        let clock = PauseClock::default();
        // Sound streams open first, so the beginning is not lost while the codec is checked.
        let mut audio = Vec::new();
        if config.system_audio {
            audio.push(AudioRecording::start(
                AudioSource::System,
                SYSTEM_TRACK,
                super::now,
            )?);
        }
        if let Some(name) = &config.microphone {
            let source = AudioSource::Microphone(name.clone());
            audio.push(AudioRecording::start(source, MIC_TRACK, super::now)?);
        }
        let origin = source_origin(&config.source)?;
        let input = InputRecording::start()?;
        let video = VideoRecording::start(&config.source, config.fps, &manifest, &clock)?;
        let start = video.info().start;
        for track in &audio {
            track.begin(start, &manifest, &clock)?;
        }
        input.begin(start, dir, origin, &clock)?;
        Ok(Self {
            manifest,
            clock,
            video,
            audio,
            input,
        })
    }

    pub fn info(&self) -> VideoStart {
        self.video.info()
    }

    pub fn dir(&self) -> &Path {
        self.manifest.dir()
    }

    /// Pauses every track now; returns the host time of the pause.
    pub fn pause(&self) -> Result<HostTime, CaptureError> {
        let at = now()?;
        let pauses = self.clock.pause(at)?;
        self.manifest.set_pauses(&pauses)?;
        Ok(at)
    }

    /// Goes on after [`Self::pause`]; returns the host time it went on.
    pub fn resume(&self) -> Result<HostTime, CaptureError> {
        let at = now()?;
        let pauses = self.clock.resume(at)?;
        self.manifest.set_pauses(&pauses)?;
        self.input.resumed(at)?;
        Ok(at)
    }

    pub fn is_paused(&self) -> Result<bool, CaptureError> {
        self.clock.is_paused()
    }

    /// Stops every track now and joins each into its MP4.
    pub fn stop(self) -> Result<RecordingSummary, CaptureError> {
        let manifest = self.manifest.clone();
        let stopped = self.end()?;
        let mut finished = manifest.get()?;
        let files = finalize(manifest.dir(), &mut finished)?;
        Ok(RecordingSummary {
            video: stopped.video,
            audio: stopped.audio,
            input: stopped.input,
            files,
            manifest: finished,
        })
    }

    /// Stops every track and deletes every file of the recording; the folder too, if nothing
    /// else is in it.
    pub fn cancel(self) -> Result<(), CaptureError> {
        let manifest = self.manifest.clone();
        let stopped = self.end();
        remove_recording(manifest.dir(), &manifest.get()?)?;
        stopped.map(drop)
    }

    fn end(mut self) -> Result<Stopped, CaptureError> {
        self.input.stop();
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
        Ok(Stopped {
            video,
            audio,
            input: self
                .input
                .join()
                .map_err(|e| format!("курсор и клавиши записаны не до конца: {e}")),
        })
    }
}
