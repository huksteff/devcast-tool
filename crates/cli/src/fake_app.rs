//! The app side of the fake editor (`fake_editor.rs`): its recording and its export queue answer
//! as the app's (`crates/app/src/remote.rs`) without a recorder or a device. The queue starts
//! with one job waiting, `p1`; an export of a run is done at once.

use std::path::PathBuf;

use dc_ops::export_jobs::{Finished, Job};
use dc_ops::live::{
    Answer, AppStatus, CaptionsState, EditorState, Once, RecordAnswer, RecordControl,
    RecordingState, Request,
};
use dc_project::ProjectDir;

pub(crate) struct FakeApp {
    recording: RecordingState,
    exports: Vec<Job>,
}

impl Default for FakeApp {
    fn default() -> Self {
        Self {
            recording: RecordingState::Idle,
            exports: vec![job(1, "очередь.mp4", "queued")],
        }
    }
}

fn job(id: u32, label: &str, state: &str) -> Job {
    Job {
        id,
        label: label.to_owned(),
        state: state.to_owned(),
        done: 0,
        total: 0,
        elapsed: 0.0,
        error: None,
    }
}

impl FakeApp {
    pub(crate) fn answer(&mut self, request: Request, dir: &ProjectDir, unsaved: bool) -> Answer {
        match request {
            Request::Record(once) => Answer::Record(self.record(dir, once)),
            Request::RecordControl(control) => Answer::Record(self.control(control)),
            Request::Export(spec) => {
                let id = self.exports.len() as u32 + 1;
                let label = spec.file_name();
                self.exports.push(job(id, &label, "done"));
                Answer::Exported(Finished::Done {
                    id,
                    path: spec.out.display().to_string(),
                    width: 1920,
                    height: 1080,
                    fps: spec.settings.fps,
                    frames: 30,
                    duration: 1.0,
                    bytes: 1_048_576,
                    codec: None,
                    seconds: 0.5,
                    stages: Default::default(),
                })
            }
            Request::Exports => Answer::Exports {
                jobs: self.exports.clone(),
            },
            Request::CancelExports { id } => {
                let mut ids = Vec::new();
                for job in &mut self.exports {
                    if job.state == "queued" && id.is_none_or(|id| id == job.id) {
                        job.state = "cancelled".to_owned();
                        ids.push(job.id);
                    }
                }
                Answer::ExportsCancelled { ids }
            }
            Request::StopCaptions { .. } => Answer::CaptionsStopped { stopped: false },
            Request::Status => Answer::Status(Box::new(AppStatus {
                recording: self.recording.clone(),
                editor: Some(EditorState {
                    root: dir.root().to_path_buf(),
                    unsaved,
                    captions: CaptionsState::Idle,
                }),
                exports: self.exports.clone(),
            })),
            _ => Answer::NotOpen,
        }
    }

    fn record(&mut self, dir: &ProjectDir, once: Once) -> RecordAnswer {
        if self.recording != RecordingState::Idle {
            return RecordAnswer::NotNow(self.recording.clone());
        }
        let root = recording_root(dir);
        if once.duration.is_some() {
            return RecordAnswer::Stopped { root };
        }
        self.recording = RecordingState::Recording {
            root: root.clone(),
            recorded: 0.0,
            paused: false,
            asking: false,
        };
        RecordAnswer::Started { root }
    }

    fn control(&mut self, control: RecordControl) -> RecordAnswer {
        let RecordingState::Recording { root, paused, .. } = &mut self.recording else {
            return RecordAnswer::NotNow(self.recording.clone());
        };
        let root = root.clone();
        match control {
            RecordControl::Pause if !*paused => {
                *paused = true;
                RecordAnswer::Paused { recorded: 1.5 }
            }
            RecordControl::Resume if *paused => {
                *paused = false;
                RecordAnswer::Resumed { recorded: 1.5 }
            }
            RecordControl::Pause | RecordControl::Resume => {
                RecordAnswer::NotNow(self.recording.clone())
            }
            RecordControl::Stop => {
                self.recording = RecordingState::Idle;
                RecordAnswer::Stopped { root }
            }
            RecordControl::Cancel => {
                self.recording = RecordingState::Idle;
                RecordAnswer::Cancelled { root }
            }
        }
    }
}

/// Where the fake app records: next to the project.
fn recording_root(dir: &ProjectDir) -> PathBuf {
    dir.root().with_file_name("Запись 1.devcast")
}
