//! Model downloads of the whole app, one per model: the settings window and every editor share
//! them ([`Downloads`] lives in `Shared`), so a second request for a model joins the running
//! download instead of writing the same `.part` file. The download runs on a thread
//! (`dc_models::ensure`: resumable, SHA-256 checked) and tells every window that joined it.
//! It is cancelled when the last of them leaves (their window closed); the `.part` file
//! continues next time.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use dc_models::ModelsError;

use crate::error::AppError;
use crate::settings::SpeechModel;

/// News of a download for one window.
#[derive(Debug, Clone)]
pub enum News {
    /// Downloaded share, 0..1.
    Progress(f32),
    Done(Result<(), Arc<ModelsError>>),
}

/// A window that waits for a download: where its news go and how to wake it.
pub struct Listener {
    /// The `Models` of the window (`Models::owner`).
    pub owner: u64,
    pub news: Sender<News>,
    pub wake: Box<dyn Fn() + Send>,
}

struct Running {
    cancel: Arc<AtomicBool>,
    /// Downloaded share 0..1; `None` until the server tells the size.
    done: Option<f32>,
    listeners: Vec<Listener>,
}

/// How a request for a model went.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Joined {
    /// A download thread was started.
    Started,
    /// The model is already being downloaded: its share so far.
    Running(Option<f32>),
}

/// The downloads of the app; clones share them.
#[derive(Clone, Default)]
pub struct Downloads(Arc<Mutex<HashMap<SpeechModel, Running>>>);

impl Downloads {
    fn lock(&self) -> MutexGuard<'_, HashMap<SpeechModel, Running>> {
        // A panic while holding the lock leaves the map as it was: every change is one insert,
        // push or remove.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `listener` waits for `model`: it joins the running download, or one is started into
    /// `dir`.
    pub fn request(
        &self,
        model: SpeechModel,
        dir: &Path,
        listener: Listener,
    ) -> Result<Joined, AppError> {
        self.join_or(model, listener, |downloads| {
            let dir = dir.to_path_buf();
            std::thread::Builder::new()
                .name("devcast-model-download".to_owned())
                .spawn(move || fetch(&downloads, model, &dir))
                .map(drop)
                .map_err(|source| AppError::Thread {
                    name: "devcast-model-download",
                    source,
                })
        })
    }

    /// [`Self::request`] with the start of the download given (tests start none).
    pub(crate) fn join_or(
        &self,
        model: SpeechModel,
        listener: Listener,
        start: impl FnOnce(Self) -> Result<(), AppError>,
    ) -> Result<Joined, AppError> {
        let mut running = self.lock();
        if let Some(download) = running.get_mut(&model) {
            download.cancel.store(false, Ordering::Relaxed);
            download.listeners.push(listener);
            return Ok(Joined::Running(download.done));
        }
        let download = Running {
            cancel: Arc::new(AtomicBool::new(false)),
            done: None,
            listeners: vec![listener],
        };
        running.insert(model, download);
        if let Err(error) = start(self.clone()) {
            running.remove(&model);
            return Err(error);
        }
        Ok(Joined::Started)
    }

    /// The window `owner` no longer waits for `model`; the last one to leave cancels it.
    pub fn leave(&self, model: SpeechModel, owner: u64) {
        let mut running = self.lock();
        if let Some(download) = running.get_mut(&model) {
            download.listeners.retain(|l| l.owner != owner);
            if download.listeners.is_empty() {
                download.cancel.store(true, Ordering::Relaxed);
            }
        }
    }

    /// The cancel flag of the download of `model` (a fresh one if there is none).
    pub(crate) fn cancel_flag(&self, model: SpeechModel) -> Arc<AtomicBool> {
        self.lock()
            .get(&model)
            .map_or_else(Arc::default, |d| Arc::clone(&d.cancel))
    }

    /// The download of `model` has `done` of the file: every listener is told.
    pub(crate) fn progress(&self, model: SpeechModel, done: f32) {
        if let Some(download) = self.lock().get_mut(&model) {
            download.done = Some(done);
            tell(&download.listeners, &News::Progress(done));
        }
    }

    /// The download of `model` ended with `outcome`; `false` when it was cancelled but a
    /// window joined since: the caller downloads again (it continues from the `.part`).
    pub(crate) fn finish(&self, model: SpeechModel, outcome: Result<(), ModelsError>) -> bool {
        let mut running = self.lock();
        let cancelled = matches!(outcome, Err(ModelsError::Cancelled));
        if let Some(download) = running.get(&model)
            && cancelled
            && !download.listeners.is_empty()
        {
            download.cancel.store(false, Ordering::Relaxed);
            return false;
        }
        if let Some(download) = running.remove(&model)
            && !cancelled
        {
            tell(&download.listeners, &News::Done(outcome.map_err(Arc::new)));
        }
        true
    }

    /// A download of `model` is running (tests).
    #[cfg(test)]
    pub(crate) fn is_running(&self, model: SpeechModel) -> bool {
        self.lock().contains_key(&model)
    }
}

/// Sends `news` to every listener and wakes the ones still there.
fn tell(listeners: &[Listener], news: &News) {
    for listener in listeners {
        if listener.news.send(news.clone()).is_ok() {
            (listener.wake)();
        }
    }
}

/// The download thread: downloads `model` into `dir`, telling whole percents and the outcome.
fn fetch(downloads: &Downloads, model: SpeechModel, dir: &Path) {
    loop {
        let cancel = downloads.cancel_flag(model);
        let mut last = None;
        let mut on_progress = |progress: dc_models::Progress| {
            let Some(total) = progress.total.filter(|t| *t > 0) else {
                return;
            };
            let percent = progress.done.saturating_mul(100) / total;
            if last != Some(percent) {
                last = Some(percent);
                downloads.progress(model, percent as f32 / 100.0);
            }
        };
        let outcome = dc_models::ensure(model.model(), dir, &mut on_progress, &cancel).map(drop);
        if downloads.finish(model, outcome) {
            return;
        }
    }
}

#[cfg(test)]
#[path = "downloads_tests.rs"]
mod tests;
