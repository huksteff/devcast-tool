//! Recording into a new project: the folder `name.dcproj` is created at the start and the
//! recording is written into its `media/recording-N/`, so after a crash it is already there.
//! At the stop the project gets one clip of the recording and the app defaults; a cancel
//! deletes the whole project folder.

use std::path::Path;

use dc_core::Project;
use dc_project::{Defaults, ProjectDir, ProjectLock};

use super::recording::{Recording, RecordingSummary};
use super::video::VIDEO_TRACK;
use crate::config::RecordingConfig;
use crate::error::CaptureError;
use crate::event::HostTime;
use crate::project::{free_recording_folder, recording_media, recording_project};

/// A recording into a project that is held locked until the stop.
pub struct ProjectRecording {
    dir: ProjectDir,
    lock: ProjectLock,
    folder: String,
    defaults: Defaults,
    recording: Recording,
}

/// A finished recording and its saved project.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedProject {
    pub dir: ProjectDir,
    pub project: Project,
    pub summary: RecordingSummary,
}

impl ProjectRecording {
    /// Creates the project folder `root` (it must not exist yet), locks it for `app` and
    /// starts recording into it; the project will get `defaults`.
    pub fn start(
        config: &RecordingConfig,
        root: &Path,
        app: &str,
        defaults: Defaults,
    ) -> Result<Self, CaptureError> {
        let dir = ProjectDir::create(root)?;
        let lock = ProjectLock::acquire(&dir, app)?;
        let folder = free_recording_folder(&dir.media_dir());
        match Recording::start(config, &dir.media_dir().join(&folder)) {
            Ok(recording) => Ok(Self {
                dir,
                lock,
                folder,
                defaults,
                recording,
            }),
            Err(error) => {
                remove(&dir, lock)?;
                Err(error)
            }
        }
    }

    pub fn dir(&self) -> &ProjectDir {
        &self.dir
    }

    pub fn recording(&self) -> &Recording {
        &self.recording
    }

    pub fn pause(&self) -> Result<HostTime, CaptureError> {
        self.recording.pause()
    }

    pub fn resume(&self) -> Result<HostTime, CaptureError> {
        self.recording.resume()
    }

    /// Finishes the recording and saves the project with it.
    pub fn stop(self) -> Result<RecordedProject, CaptureError> {
        let size = self.recording.info().size;
        let summary = self.recording.stop()?;
        let video = summary
            .manifest
            .tracks
            .iter()
            .find(|t| t.name == VIDEO_TRACK);
        let video = video
            .ok_or_else(|| CaptureError::EmptyRecording(self.dir.media_dir().join(&self.folder)))?;
        let media = recording_media(
            &self.folder,
            &video.final_file(),
            size,
            summary.video.end,
            video.rate,
        );
        let project = recording_project(media, &self.defaults)?;
        self.dir.save(&project)?;
        drop(self.lock);
        Ok(RecordedProject {
            dir: self.dir,
            project,
            summary,
        })
    }

    /// Stops and deletes the whole project folder with the recording.
    pub fn cancel(self) -> Result<(), CaptureError> {
        let cancelled = self.recording.cancel();
        remove(&self.dir, self.lock)?;
        cancelled
    }
}

/// Deletes the project folder this recording created.
fn remove(dir: &ProjectDir, lock: ProjectLock) -> Result<(), CaptureError> {
    drop(lock);
    std::fs::remove_dir_all(dir.root()).map_err(CaptureError::file("удалить", dir.root()))
}
