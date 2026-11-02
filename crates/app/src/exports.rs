//! The export queue of the whole app (task 7.31): one `dc_encode::ExportQueue` in `Shared`, the
//! editor holds a clone, so an export goes on after the editor closes and while recording.
//! A job is `dc_ops::export::run` of a `Spec` — the job of `devcast export`. The queue's watch
//! wakes only the main window (decision 002), at the start, at each whole percent and at the
//! end; the main window passes it on to the editor (`take_news`). The same goes for the encoder
//! check of the dialog's «Кодирует» (`encoder`): once per app run, on its own thread.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use dc_encode::{Codec, ExportQueue, Job, JobState, JobStatus};
use dc_ops::export::Spec;

use crate::error::AppError;

/// Turns a spec into the job the queue runs: the real export, or a stand-in in tests.
pub type Runner = Arc<dyn Fn(Spec) -> Job + Send + Sync>;

/// Finds the codec an MP4 export starts with (`dc_encode::export_codec`), or a stand-in in
/// tests; the error says what to do. It may take a second: it runs on its own thread.
pub type Probe = Arc<dyn Fn() -> Result<Codec, String> + Send + Sync>;

/// What the encoder check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Encoder {
    Checking,
    Ready(Codec),
    Failed(String),
}

type Wake = Arc<dyn Fn() + Send + Sync>;

/// The export queue; clones share it.
#[derive(Clone)]
pub struct Exports(Arc<Inner>);

struct Inner {
    queue: ExportQueue,
    runner: Runner,
    probe: Probe,
    wake: Wake,
    news: Arc<AtomicBool>,
    /// Finished jobs whose toast was shown.
    told: Mutex<HashSet<u32>>,
    /// `None` until the first `encoder` call starts the check.
    encoder: Mutex<Option<Encoder>>,
}

impl std::fmt::Debug for Exports {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Exports").finish_non_exhaustive()
    }
}

impl Exports {
    /// The queue whose jobs `runner` makes, with the real encoder check; `wake` repaints the
    /// main window.
    pub fn new(runner: Runner, wake: impl Fn() + Send + Sync + 'static) -> Result<Self, AppError> {
        let probe: Probe = Arc::new(|| dc_encode::export_codec().map_err(|e| e.to_string()));
        Self::with_probe(runner, probe, wake)
    }

    /// [`Exports::new`] whose encoder check is `probe`.
    pub fn with_probe(
        runner: Runner,
        probe: Probe,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, AppError> {
        let news = Arc::new(AtomicBool::new(false));
        let wake: Wake = Arc::new(wake);
        let heard = Arc::clone(&news);
        let waker = Arc::clone(&wake);
        let watch = move |status: &JobStatus| {
            if worth_a_frame(&status.state) {
                heard.store(true, Ordering::SeqCst);
                waker();
            }
        };
        let queue = ExportQueue::watched(Box::new(watch)).map_err(|e| AppError::Thread {
            name: "devcast-export",
            source: std::io::Error::other(e.to_string()),
        })?;
        Ok(Self(Arc::new(Inner {
            queue,
            runner,
            probe,
            wake,
            news,
            told: Mutex::new(HashSet::new()),
            encoder: Mutex::new(None),
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

    /// The encoder an MP4 export starts with. The first call starts the check on its own
    /// thread and says `Checking`; its answer sets the news and wakes the main window.
    pub fn encoder(&self) -> Encoder {
        let mut slot = self
            .0
            .encoder
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(known) = slot.as_ref() {
            return known.clone();
        }
        let inner = Arc::clone(&self.0);
        let check = move || {
            let found = match (inner.probe)() {
                Ok(codec) => Encoder::Ready(codec),
                Err(error) => Encoder::Failed(error),
            };
            *inner.encoder.lock().unwrap_or_else(PoisonError::into_inner) = Some(found);
            inner.news.store(true, Ordering::SeqCst);
            (inner.wake)();
        };
        let spawned = std::thread::Builder::new()
            .name("devcast-codec-check".to_owned())
            .spawn(check);
        let now = match spawned {
            Ok(_) => Encoder::Checking,
            Err(source) => Encoder::Failed(
                AppError::Thread {
                    name: "devcast-codec-check",
                    source,
                }
                .to_string(),
            ),
        };
        *slot = Some(now.clone());
        now
    }

    #[cfg(test)]
    pub fn wait_idle(&self) {
        self.0.queue.wait_idle();
    }

    /// Waits until the encoder check has an answer.
    #[cfg(test)]
    pub fn wait_encoder(&self) -> Encoder {
        loop {
            match self.encoder() {
                Encoder::Checking => std::thread::yield_now(),
                known => return known,
            }
        }
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
