//! The export queue of the whole app (task 7.31): one `dc_encode::ExportQueue` in `Shared`, the
//! editor holds a clone, so an export goes on after the editor closes and while recording.
//! A job is `dc_ops::export::run` of a `Spec` — the job of `devcast export`. The queue's watch
//! wakes only the main window (decision 002), at the start, at each whole percent and at the
//! end; the main window passes it on to the editor (`take_news`).

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use dc_encode::{ExportQueue, Job, JobState, JobStatus};
use dc_ops::export::Spec;

use crate::error::AppError;

/// Turns a spec into the job the queue runs: the real export, or a stand-in in tests.
pub type Runner = Arc<dyn Fn(Spec) -> Job + Send + Sync>;

/// The export queue; clones share it.
#[derive(Clone)]
pub struct Exports(Arc<Inner>);

struct Inner {
    queue: ExportQueue,
    runner: Runner,
    news: Arc<AtomicBool>,
    /// Finished jobs whose toast was shown.
    told: Mutex<HashSet<u32>>,
}

impl std::fmt::Debug for Exports {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Exports").finish_non_exhaustive()
    }
}

impl Exports {
    /// The queue whose jobs `runner` makes; `wake` repaints the main window.
    pub fn new(runner: Runner, wake: impl Fn() + Send + Sync + 'static) -> Result<Self, AppError> {
        let news = Arc::new(AtomicBool::new(false));
        let heard = Arc::clone(&news);
        let watch = move |status: &JobStatus| {
            if worth_a_frame(&status.state) {
                heard.store(true, Ordering::SeqCst);
                wake();
            }
        };
        let queue = ExportQueue::watched(Box::new(watch)).map_err(|e| AppError::Thread {
            name: "devcast-export",
            source: std::io::Error::other(e.to_string()),
        })?;
        Ok(Self(Arc::new(Inner {
            queue,
            runner,
            news,
            told: Mutex::new(HashSet::new()),
        })))
    }

    /// The real export on the app's device.
    pub fn of_gpu(
        gpu: Arc<dc_render::Gpu>,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, AppError> {
        let runner: Runner = Arc::new(move |spec: Spec| -> Job {
            let gpu = Arc::clone(&gpu);
            Box::new(move |progress| dc_ops::export::run(&gpu, &spec, progress))
        });
        Self::new(runner, wake)
    }

    /// Puts `spec` at the end of the queue; its label is the file name.
    pub fn push(&self, spec: Spec) -> u32 {
        let label = spec.file_name();
        let job = (self.0.runner)(spec);
        self.0.queue.push(label, job)
    }

    pub fn status(&self) -> Vec<JobStatus> {
        self.0.queue.status()
    }

    /// A job is waiting or running.
    pub fn busy(&self) -> bool {
        self.status()
            .iter()
            .any(|s| matches!(s.state, JobState::Queued | JobState::Running { .. }))
    }

    /// Cancels job `id` or (`None`) all of them; the ids cancelled.
    pub fn cancel(&self, id: Option<u32>) -> Vec<u32> {
        self.0.queue.cancel(id)
    }

    /// Waits until job `id` has ended; `None` when the queue does not know it.
    pub fn wait(&self, id: u32) -> Option<JobState> {
        self.0.queue.wait(id)
    }

    /// The queue changed since the last call.
    pub fn take_news(&self) -> bool {
        self.0.news.swap(false, Ordering::SeqCst)
    }

    /// Done and failed jobs not told before: each is told once, to one window.
    pub fn untold(&self) -> Vec<JobStatus> {
        let mut told = self.0.told.lock().unwrap_or_else(PoisonError::into_inner);
        self.status()
            .into_iter()
            .filter(|s| matches!(s.state, JobState::Done { .. } | JobState::Failed(_)))
            .filter(|s| told.insert(s.id))
            .collect()
    }

    #[cfg(test)]
    pub fn wait_idle(&self) {
        self.0.queue.wait_idle();
    }
}

/// A change the window shows: anything but a frame inside the same whole percent.
fn worth_a_frame(state: &JobState) -> bool {
    let JobState::Running { progress, .. } = state else {
        return true;
    };
    let percent = |done: u64| done * 100 / progress.total;
    progress.done <= 1 || percent(progress.done) != percent(progress.done - 1)
}
