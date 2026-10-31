//! Export queue (SCOPE, «Экспорт»): exports run one after another on a worker thread, so the
//! caller keeps editing and recording; each job reports its progress and can be cancelled.

use std::collections::VecDeque;
use std::ops::ControlFlow;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::error::EncodeError;
use crate::export::{Exported, Progress, ProgressFn};

/// One export: runs on the worker, reports progress, stops when it returns `Break`.
pub type Job = Box<dyn FnOnce(&mut ProgressFn<'_>) -> Result<Exported, EncodeError> + Send>;

/// Where a job is.
#[derive(Debug, Clone, PartialEq)]
pub enum JobState {
    Queued,
    Running {
        progress: Progress,
        /// Time since the job started.
        elapsed: Duration,
    },
    Done {
        exported: Box<Exported>,
        elapsed: Duration,
    },
    Failed(String),
    Cancelled,
}

impl JobState {
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Done { .. } | Self::Failed(_) | Self::Cancelled)
    }
}

/// A job as the queue shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct JobStatus {
    /// Number in the queue, from 1.
    pub id: u32,
    /// What the caller named it: usually the output file.
    pub label: String,
    pub state: JobState,
}

pub struct ExportQueue {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
struct State {
    jobs: Vec<Entry>,
    waiting: VecDeque<(u32, Job)>,
    next_id: u32,
    /// The queue is dropped: the worker stops.
    closed: bool,
}

struct Entry {
    status: JobStatus,
    cancel: bool,
    started: Option<Instant>,
}

impl ExportQueue {
    pub fn new() -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        });
        let worker = std::thread::Builder::new()
            .name("devcast-export".to_owned())
            .spawn({
                let shared = Arc::clone(&shared);
                move || work(&shared)
            })
            .ok();
        Self { shared, worker }
    }

    /// Puts `job` at the end of the queue; returns its id.
    pub fn push(&self, label: String, job: Job) -> u32 {
        let mut state = self.shared.lock();
        state.next_id += 1;
        let id = state.next_id;
        state.jobs.push(Entry {
            status: JobStatus {
                id,
                label,
                state: JobState::Queued,
            },
            cancel: false,
            started: None,
        });
        state.waiting.push_back((id, job));
        drop(state);
        self.shared.changed.notify_all();
        id
    }

    pub fn status(&self) -> Vec<JobStatus> {
        let state = self.shared.lock();
        state
            .jobs
            .iter()
            .map(|entry| {
                let mut status = entry.status.clone();
                if let (JobState::Running { elapsed, .. }, Some(started)) =
                    (&mut status.state, entry.started)
                {
                    *elapsed = started.elapsed();
                }
                status
            })
            .collect()
    }

    /// Cancels job `id` (`None`: every unfinished job); returns the ids it cancelled. A queued
    /// job never starts, a running one stops at its next frame.
    pub fn cancel(&self, id: Option<u32>) -> Vec<u32> {
        let mut state = self.shared.lock();
        let mut cancelled = Vec::new();
        for entry in &mut state.jobs {
            let chosen = id.is_none_or(|id| id == entry.status.id);
            if chosen && !entry.status.state.is_finished() && !entry.cancel {
                entry.cancel = true;
                cancelled.push(entry.status.id);
            }
        }
        drop(state);
        self.shared.changed.notify_all();
        cancelled
    }

    /// Waits until job `id` has finished and returns how; `None` for an unknown id.
    pub fn wait(&self, id: u32) -> Option<JobState> {
        let mut state = self.shared.lock();
        loop {
            let entry = state.jobs.iter().find(|e| e.status.id == id)?;
            if entry.status.state.is_finished() {
                return Some(entry.status.state.clone());
            }
            state = self.shared.wait(state);
        }
    }

    /// Waits until every job pushed so far has finished.
    pub fn wait_idle(&self) {
        let mut state = self.shared.lock();
        while state.jobs.iter().any(|e| !e.status.state.is_finished()) {
            state = self.shared.wait(state);
        }
    }
}

impl Default for ExportQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ExportQueue {
    fn drop(&mut self) {
        self.cancel(None);
        {
            let mut state = self.shared.lock();
            state.closed = true;
        }
        self.shared.changed.notify_all();
        if let Some(worker) = self.worker.take() {
            // A job that panicked has already reported nothing more can be done with it.
            let _ = worker.join();
        }
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn wait<'a>(&self, guard: MutexGuard<'a, State>) -> MutexGuard<'a, State> {
        self.changed
            .wait(guard)
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn set(&self, id: u32, change: impl FnOnce(&mut Entry)) {
        let mut state = self.lock();
        if let Some(entry) = state.jobs.iter_mut().find(|e| e.status.id == id) {
            change(entry);
        }
        drop(state);
        self.changed.notify_all();
    }
}

fn work(shared: &Shared) {
    loop {
        let (id, job) = {
            let mut state = shared.lock();
            loop {
                if let Some(next) = state.waiting.pop_front() {
                    break next;
                }
                if state.closed {
                    return;
                }
                state = shared.wait(state);
            }
        };
        run(shared, id, job);
    }
}

fn run(shared: &Shared, id: u32, job: Job) {
    let started = Instant::now();
    let mut skip = false;
    shared.set(id, |entry| {
        skip = entry.cancel;
        entry.started = Some(started);
        entry.status.state = match skip {
            true => JobState::Cancelled,
            false => JobState::Running {
                progress: Progress { done: 0, total: 0 },
                elapsed: Duration::ZERO,
            },
        };
    });
    if skip {
        return;
    }
    let mut progress = |progress: Progress| {
        let mut cancel = false;
        shared.set(id, |entry| {
            cancel = entry.cancel;
            entry.status.state = JobState::Running {
                progress,
                elapsed: started.elapsed(),
            };
        });
        match cancel {
            true => ControlFlow::Break(()),
            false => ControlFlow::Continue(()),
        }
    };
    let result = job(&mut progress);
    shared.set(id, |entry| {
        entry.status.state = match result {
            Ok(exported) => JobState::Done {
                exported: Box::new(exported),
                elapsed: started.elapsed(),
            },
            Err(EncodeError::Cancelled) => JobState::Cancelled,
            Err(error) => JobState::Failed(error.to_string()),
        };
    });
}
