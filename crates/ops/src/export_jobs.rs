//! Export jobs as the callers see them: `devcast export status` lists [`Job`]s and a finished
//! `devcast export` prints [`Finished`]. One shape for both queues: the queue of `devcast export`
//! (channel `devcast-export-<user>`) and the app's (channel `devcast-app-<user>`, `live.rs`).

use std::collections::BTreeMap;

use dc_encode::{JobState, JobStatus};
use serde::{Deserialize, Serialize};

/// How a job ended, as the caller prints it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Finished {
    Done {
        id: u32,
        path: String,
        width: u32,
        height: u32,
        fps: u32,
        frames: u64,
        /// Length of the video, s.
        duration: f64,
        bytes: u64,
        codec: Option<String>,
        /// How long the export took, s.
        seconds: f64,
        /// Busy time of every export stage, s, to a thousandth (`dc_encode::export::Stages`).
        stages: BTreeMap<String, f64>,
    },
    Failed {
        id: u32,
        message: String,
    },
    Cancelled {
        id: u32,
    },
}

/// A job as `export status` prints it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub id: u32,
    pub label: String,
    /// queued, running, done, failed, cancelled.
    pub state: String,
    pub done: u64,
    pub total: u64,
    /// Since the start, s.
    pub elapsed: f64,
    pub error: Option<String>,
}

impl Job {
    pub fn of(status: &JobStatus) -> Self {
        let mut job = Self {
            id: status.id,
            label: status.label.clone(),
            state: String::new(),
            done: 0,
            total: 0,
            elapsed: 0.0,
            error: None,
        };
        job.state = match &status.state {
            JobState::Queued => "queued",
            JobState::Running { progress, elapsed } => {
                (job.done, job.total) = (progress.done, progress.total);
                job.elapsed = elapsed.as_secs_f64();
                "running"
            }
            JobState::Done { exported, elapsed } => {
                (job.done, job.total) = (exported.frames, exported.frames);
                job.elapsed = elapsed.as_secs_f64();
                "done"
            }
            JobState::Failed(message) => {
                job.error = Some(message.clone());
                "failed"
            }
            JobState::Cancelled => "cancelled",
        }
        .to_owned();
        job
    }
}

/// How job `id` ended; a state that is not an end (the queue lost the job) counts as cancelled.
pub fn finished(id: u32, state: JobState) -> Finished {
    match state {
        JobState::Done { exported, elapsed } => Finished::Done {
            id,
            path: exported.path.display().to_string(),
            width: exported.size.width,
            height: exported.size.height,
            fps: exported.fps,
            frames: exported.frames,
            duration: exported.duration.as_secs_f64(),
            bytes: exported.bytes,
            codec: exported.codec.map(|c| c.to_string()),
            seconds: elapsed.as_secs_f64(),
            stages: exported
                .stages
                .named()
                .into_iter()
                .map(|(name, time)| {
                    let seconds = (time.as_secs_f64() * 1000.0).round() / 1000.0;
                    (name.to_owned(), seconds)
                })
                .collect(),
        },
        JobState::Failed(message) => Finished::Failed { id, message },
        JobState::Queued | JobState::Running { .. } | JobState::Cancelled => {
            Finished::Cancelled { id }
        }
    }
}
