//! Whisper models of the settings (mockup 5c): which ones are on disk and the download with
//! progress in the model's card. The download runs on a worker thread (`dc_transcribe::ensure`:
//! resumable, SHA-256 checked) and wakes the main window; closing the window cancels it, and the
//! `.part` file continues next time.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};

use crate::error::AppError;
use crate::settings::SpeechModel;

/// What the card of a model shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModelState {
    Missing,
    Ready,
    /// Downloaded share 0..1; `None` until the server tells the size.
    Downloading(Option<f32>),
}

enum Message {
    Progress(f32),
    Done(Result<(), dc_transcribe::TranscribeError>),
}

struct Download {
    receiver: Receiver<Message>,
    cancel: Arc<AtomicBool>,
    done: Option<f32>,
}

/// The models folder and the downloads started from the settings window.
pub struct Models {
    dir: Option<PathBuf>,
    ready: Vec<SpeechModel>,
    downloads: HashMap<SpeechModel, Download>,
}

impl Models {
    /// Models in `dir`; `None`: the folder is unknown, nothing is ready or offered.
    pub fn new(dir: Option<PathBuf>) -> Self {
        let ready = dir.as_ref().map_or_else(Vec::new, |dir| {
            SpeechModel::ALL
                .into_iter()
                .filter(|m| dir.join(m.model().file_name()).is_file())
                .collect()
        });
        Self {
            dir,
            ready,
            downloads: HashMap::new(),
        }
    }

    /// The folder of the app, `%APPDATA%\DevCast\models`.
    pub fn of_app() -> Self {
        match dc_transcribe::models_dir() {
            Ok(dir) => Self::new(Some(dir)),
            Err(error) => {
                tracing::warn!("папка моделей неизвестна, «Скачать» не показан: {error}");
                Self::new(None)
            }
        }
    }

    pub fn state(&self, model: SpeechModel) -> ModelState {
        match self.downloads.get(&model) {
            Some(download) => ModelState::Downloading(download.done),
            None if self.ready.contains(&model) => ModelState::Ready,
            None => ModelState::Missing,
        }
    }

    /// «Скачать» is offered: the folder is known.
    pub fn can_download(&self) -> bool {
        self.dir.is_some()
    }

    /// Starts downloading `model`; the worker calls `wake` on each whole percent (it wakes the
    /// main window, app CLAUDE.md).
    pub fn download(
        &mut self,
        wake: impl Fn() + Send + 'static,
        model: SpeechModel,
    ) -> Result<(), AppError> {
        let Some(dir) = self.dir.clone() else {
            return Ok(());
        };
        if self.downloads.contains_key(&model) {
            return Ok(());
        }
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&cancel);
        std::thread::Builder::new()
            .name("devcast-model-download".to_owned())
            .spawn(move || fetch(model, &dir, &sender, &stop, &wake))
            .map_err(|source| AppError::Thread {
                name: "devcast-model-download",
                source,
            })?;
        let download = Download {
            receiver,
            cancel,
            done: None,
        };
        self.downloads.insert(model, download);
        Ok(())
    }

    /// Takes the worker's news; a failed download is returned once.
    pub fn poll(&mut self) -> Result<(), AppError> {
        let mut result = Ok(());
        let mut finished = Vec::new();
        for (model, download) in &mut self.downloads {
            for message in download.receiver.try_iter() {
                match message {
                    Message::Progress(done) => download.done = Some(done),
                    Message::Done(Ok(())) => {
                        finished.push(*model);
                        self.ready.push(*model);
                    }
                    Message::Done(Err(error)) => {
                        finished.push(*model);
                        result = Err(AppError::Model(error));
                    }
                }
            }
        }
        for model in finished {
            self.downloads.remove(&model);
        }
        result
    }
}

impl Drop for Models {
    fn drop(&mut self) {
        for download in self.downloads.values() {
            download.cancel.store(true, Ordering::Relaxed);
        }
    }
}

/// The worker: downloads `model` into `dir`, sending whole percents and the outcome.
fn fetch(
    model: SpeechModel,
    dir: &Path,
    sender: &Sender<Message>,
    cancel: &AtomicBool,
    wake: &dyn Fn(),
) {
    // The receiver goes with its window (settings, editor), which cancels the download first.
    let tell = |message: Message| {
        if sender.send(message).is_ok() {
            wake();
        }
    };
    let mut last = None;
    let mut on_progress = |progress: dc_transcribe::Progress| {
        let Some(total) = progress.total.filter(|t| *t > 0) else {
            return;
        };
        let percent = progress.done.saturating_mul(100) / total;
        if last != Some(percent) {
            last = Some(percent);
            tell(Message::Progress(percent as f32 / 100.0));
        }
    };
    let outcome = dc_transcribe::ensure(model.model(), dir, &mut on_progress, cancel).map(|_| ());
    if !matches!(outcome, Err(dc_transcribe::TranscribeError::Cancelled)) {
        tell(Message::Done(outcome));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_models_are_the_files_in_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let file = SpeechModel::Fast.model().file_name();
        std::fs::write(dir.path().join(file), b"model").unwrap();
        let models = Models::new(Some(dir.path().to_path_buf()));
        assert_eq!(models.state(SpeechModel::Fast), ModelState::Ready);
        assert_eq!(models.state(SpeechModel::Accurate), ModelState::Missing);
        assert!(models.can_download());
        let nowhere = Models::new(None);
        assert_eq!(nowhere.state(SpeechModel::Fast), ModelState::Missing);
        assert!(!nowhere.can_download());
    }

    #[test]
    fn progress_and_the_end_of_a_download() {
        let mut models = Models::new(Some(PathBuf::from("unused")));
        let (sender, receiver) = mpsc::channel();
        let download = Download {
            receiver,
            cancel: Arc::new(AtomicBool::new(false)),
            done: None,
        };
        models.downloads.insert(SpeechModel::Accurate, download);
        let state = models.state(SpeechModel::Accurate);
        assert_eq!(state, ModelState::Downloading(None));
        sender.send(Message::Progress(0.42)).unwrap();
        models.poll().unwrap();
        let state = models.state(SpeechModel::Accurate);
        assert_eq!(state, ModelState::Downloading(Some(0.42)));
        sender.send(Message::Done(Ok(()))).unwrap();
        models.poll().unwrap();
        assert_eq!(models.state(SpeechModel::Accurate), ModelState::Ready);
    }
}
