//! Whisper models of a window (settings mockup 5c, the editor's «Создать субтитры»): which ones
//! are on disk and the downloads it waits for, with progress in the model's card. The downloads
//! themselves are the app's shared ones (`downloads.rs`): two windows asking for one model wait
//! for one download. Closing the window leaves its downloads; the last window to leave cancels,
//! and the `.part` file continues next time.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};

use crate::downloads::{Downloads, Joined, Listener, News};
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

struct Waiting {
    news: Receiver<News>,
    done: Option<f32>,
}

/// The models folder and the downloads one window waits for.
pub struct Models {
    dir: Option<PathBuf>,
    ready: Vec<SpeechModel>,
    downloads: Downloads,
    /// Tells this window's waits apart in the shared downloads.
    owner: u64,
    waiting: HashMap<SpeechModel, Waiting>,
}

impl Models {
    /// Models in `dir`, downloaded through `downloads`; `None`: the folder is unknown, nothing
    /// is ready or offered.
    pub fn new(dir: Option<PathBuf>, downloads: Downloads) -> Self {
        static NEXT_OWNER: AtomicU64 = AtomicU64::new(0);
        let ready = dir.as_ref().map_or_else(Vec::new, |dir| {
            SpeechModel::ALL
                .into_iter()
                .filter(|m| dir.join(m.model().file_name()).is_file())
                .collect()
        });
        Self {
            dir,
            ready,
            downloads,
            owner: NEXT_OWNER.fetch_add(1, Ordering::Relaxed),
            waiting: HashMap::new(),
        }
    }

    /// The folder of the app, `%APPDATA%\DevCast\models`.
    pub fn of_app(downloads: Downloads) -> Self {
        match dc_models::models_dir() {
            Ok(dir) => Self::new(Some(dir), downloads),
            Err(error) => {
                tracing::warn!("папка моделей неизвестна, «Скачать» не показан: {error}");
                Self::new(None, downloads)
            }
        }
    }

    pub fn state(&self, model: SpeechModel) -> ModelState {
        match self.waiting.get(&model) {
            Some(waiting) => ModelState::Downloading(waiting.done),
            None if self.ready.contains(&model) => ModelState::Ready,
            None => ModelState::Missing,
        }
    }

    /// «Скачать» is offered: the folder is known.
    pub fn can_download(&self) -> bool {
        self.dir.is_some()
    }

    /// Waits for `model`: joins its download or starts one; `wake` is called on each whole
    /// percent (it wakes the main window, app CLAUDE.md).
    pub fn download(
        &mut self,
        wake: impl Fn() + Send + 'static,
        model: SpeechModel,
    ) -> Result<(), AppError> {
        let Some(dir) = self.dir.clone() else {
            return Ok(());
        };
        if self.waiting.contains_key(&model) {
            return Ok(());
        }
        let (sender, news) = mpsc::channel();
        let listener = Listener {
            owner: self.owner,
            news: sender,
            wake: Box::new(wake),
        };
        let done = match self.downloads.request(model, &dir, listener)? {
            Joined::Started => None,
            Joined::Running(done) => done,
        };
        self.waiting.insert(model, Waiting { news, done });
        Ok(())
    }

    /// Takes the news of the downloads; a failed download is returned once.
    pub fn poll(&mut self) -> Result<(), AppError> {
        let mut result = Ok(());
        let mut finished = Vec::new();
        for (model, waiting) in &mut self.waiting {
            for news in waiting.news.try_iter() {
                match news {
                    News::Progress(done) => waiting.done = Some(done),
                    News::Done(Ok(())) => {
                        finished.push(*model);
                        self.ready.push(*model);
                    }
                    News::Done(Err(error)) => {
                        finished.push(*model);
                        result = Err(AppError::Model(error));
                    }
                }
            }
        }
        for model in finished {
            self.waiting.remove(&model);
        }
        result
    }
}

impl Drop for Models {
    fn drop(&mut self) {
        for model in self.waiting.keys() {
            self.downloads.leave(*model, self.owner);
        }
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
        let models = Models::new(Some(dir.path().to_path_buf()), Downloads::default());
        assert_eq!(models.state(SpeechModel::Fast), ModelState::Ready);
        assert_eq!(models.state(SpeechModel::Accurate), ModelState::Missing);
        assert!(models.can_download());
        let nowhere = Models::new(None, Downloads::default());
        assert_eq!(nowhere.state(SpeechModel::Fast), ModelState::Missing);
        assert!(!nowhere.can_download());
    }
}
