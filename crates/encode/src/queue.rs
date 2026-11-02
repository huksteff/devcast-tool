//! Export queue (SCOPE, «Экспорт»): exports run one after another on a worker thread, so the
//! caller keeps editing and recording; each job reports its progress and can be cancelled. A
//! job that panics fails alone: the queue goes on with the next one.

use std::collections::VecDeque;
use std::ops::ControlFlow;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::error::EncodeError;
use crate::export::{Exported, Progress, ProgressFn};

/// One export: runs on the worker, reports progress, stops when it returns `Break`.
pub type Job = Box<dyn FnOnce(&mut ProgressFn<'_>) -> Result<Exported, EncodeError> + Send>;

/// Hears every change the worker makes to a job (start, each frame, end), after the change: a
/// window wakes on it instead of polling the queue.
pub type Watch = Box<dyn Fn(&JobStatus) + Send + Sync>;

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
    /// Taken when the queue is dropped.
    worker: Option<JoinHandle<()>>,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    watch: Option<Watch>,
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
    pub fn new() -> Result<Self, EncodeError> {
        Self::start(None)
    }

    /// [`ExportQueue::new`] whose `watch` hears every change the worker makes to a job.
    pub fn watched(watch: Watch) -> Result<Self, EncodeError> {
        Self::start(Some(watch))
    }

    fn start(watch: Option<Watch>) -> Result<Self, EncodeError> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            watch,
        });
        let worker = std::thread::Builder::new()
            .name("devcast-export".to_owned())
            .spawn({
                let shared = Arc::clone(&shared);
                move || work(&shared)
            })
            .map_err(EncodeError::Thread)?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
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

impl Drop for ExportQueue {
    fn drop(&mut self) {
        self.cancel(None);
        {
            let mut state = self.shared.lock();
            state.closed = true;
        }
        self.shared.changed.notify_all();
        if let Some(worker) = self.worker.take() {
            // Panics of jobs are caught in `run` and reported as `Failed`; the worker itself
            // has nothing more to report once the queue is gone.
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
        let status = state
            .jobs
            .iter_mut()
            .find(|e| e.status.id == id)
            .and_then(|entry| {
                change(entry);
                self.watch.as_ref().map(|_| entry.status.clone())
            });
        drop(state);
        self.changed.notify_all();
        if let (Some(watch), Some(status)) = (&self.watch, status) {
            watch(&status);
        }
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
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| job(&mut progress)));
    shared.set(id, |entry| {
        entry.status.state = match result {
            Ok(Ok(exported)) => JobState::Done {
                exported: Box::new(exported),
                elapsed: started.elapsed(),
            },
            Ok(Err(EncodeError::Cancelled)) => JobState::Cancelled,
            Ok(Err(error)) => JobState::Failed(error.to_string()),
            Err(panic) => JobState::Failed(panicked(panic.as_ref())),
        };
    });
}

/// The message of a job's panic.
fn panicked(panic: &(dyn std::any::Any + Send)) -> String {
    let reason = panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("причина неизвестна");
    format!("экспорт упал внутри программы ({reason}), файл не записан: сообщи разработчикам")
}
