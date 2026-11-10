//! Recording into a new project: the folder `name.devcast` with the app defaults is created at
//! the start and the recording is written into its `media/recording-N/`, so after a crash it is
//! already there (and [`crate::project::recover_recordings`] finishes it). At the stop the
//! project gets the clip of the recording; a cancel deletes the whole project folder.

use std::path::Path;

use dc_core::Project;
use dc_project::{Defaults, ProjectDir, ProjectLock};

use super::recording::{Recording, RecordingSummary};
use crate::config::RecordingConfig;
use crate::error::CaptureError;
use crate::event::HostTime;
use crate::project::{add_recording, free_recording_folder};

/// A recording into a project that is held locked until the stop.
pub struct ProjectRecording {
    dir: ProjectDir,
    lock: ProjectLock,
    folder: String,
    project: Project,
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
        super::disk::room_to_start(root)?;
        let dir = ProjectDir::create(root)?;
        let lock = ProjectLock::acquire(&dir, app)?;
        let mut project = Project::default();
        defaults.apply(&mut project);
        dir.save(&project)?;
        let folder = free_recording_folder(&dir.media_dir());
        match Recording::start(config, &dir.media_dir().join(&folder)) {
            Ok(recording) => Ok(Self {
                dir,
                lock,
                folder,
                project,
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
    pub fn stop(mut self) -> Result<RecordedProject, CaptureError> {
        let summary = self.recording.stop()?;
        add_recording(&mut self.project, &self.folder, &summary.manifest)?;
        self.dir.save(&self.project)?;
        drop(self.lock);
        Ok(RecordedProject {
            dir: self.dir,
            project: self.project,
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
