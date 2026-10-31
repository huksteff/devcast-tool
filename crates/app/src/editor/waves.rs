//! Peaks of the waves on the sound rows (7.11), read on the thread `devcast-peaks`: the UI never
//! waits for them. A sound row is drawn plain until its peaks come; the thread wakes the main
//! window (app CLAUDE.md), and the main window wakes the editor (`take_news`). The peaks of a
//! track are built once and kept in the project cache by `dc_audio::cached_peaks`; the window
//! keeps every source it read for as long as it is open (a few per recording and sound file).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use dc_audio::peaks::{Peaks, cached_peaks};
use dc_audio::{file_track, tracks};
use dc_project::ProjectDir;
use eframe::egui;

use super::timeline::Source;

/// The peaks of a source or why there are none.
pub type Answer = (Source, Result<Peaks, String>);

#[derive(Debug)]
enum Slot {
    Waiting,
    Ready(Arc<Peaks>),
    /// Unreadable: the row stays plain; the reason is in the log.
    Failed,
}

#[derive(Debug, Default)]
pub struct Waves {
    slots: Vec<(Source, Slot)>,
    worker: Option<Worker>,
}

#[derive(Debug)]
struct Worker {
    requests: Sender<Source>,
    answers: Receiver<Answer>,
    news: Arc<AtomicBool>,
}

impl Waves {
    /// Starts the thread that reads the peaks of the project in `dir`; it wakes the main window
    /// of `ctx` after every answer.
    pub fn start(&mut self, dir: ProjectDir, ctx: egui::Context) -> std::io::Result<()> {
        let wake = move || ctx.request_repaint_of(egui::ViewportId::ROOT);
        self.start_with(move |source| load(&dir, source), wake)
    }

    /// [`Waves::start`] with its own reading of the peaks: the fake of the tests.
    pub fn start_with(
        &mut self,
        load: impl Fn(&Source) -> Result<Peaks, String> + Send + 'static,
        wake: impl Fn() + Send + 'static,
    ) -> std::io::Result<()> {
        let (requests, inbox) = channel::<Source>();
        let (outbox, answers) = channel::<Answer>();
        let news = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&news);
        std::thread::Builder::new()
            .name("devcast-peaks".to_owned())
            .spawn(move || {
                for source in inbox {
                    let peaks = load(&source);
                    if outbox.send((source, peaks)).is_err() {
                        return;
                    }
                    flag.store(true, Ordering::Release);
                    wake();
                }
            })?;
        let worker = Worker {
            requests,
            answers,
            news,
        };
        for (source, _) in self
            .slots
            .iter()
            .filter(|(_, s)| matches!(s, Slot::Waiting))
        {
            // A send fails only when the thread is gone: those rows stay plain.
            let _ = worker.requests.send(source.clone());
        }
        self.worker = Some(worker);
        Ok(())
    }

    /// Asks for the peaks of `sources` it does not have yet.
    pub fn want<'a>(&mut self, sources: impl Iterator<Item = &'a Source>) {
        for source in sources {
            if self.slots.iter().any(|(s, _)| s == source) {
                continue;
            }
            self.slots.push((source.clone(), Slot::Waiting));
            if let Some(worker) = &self.worker
                && worker.requests.send(source.clone()).is_err()
            {
                self.arrive(
                    source.clone(),
                    Err("поток чтения волны остановлен".to_owned()),
                );
            }
        }
    }

    /// Takes the answers of the thread; whether any came.
    pub fn receive(&mut self) -> bool {
        let Some(worker) = &self.worker else {
            return false;
        };
        let answers: Vec<Answer> = worker.answers.try_iter().collect();
        let any = !answers.is_empty();
        for (source, peaks) in answers {
            self.arrive(source, peaks);
        }
        any
    }

    /// The peaks of `source` came (or failed).
    pub fn arrive(&mut self, source: Source, peaks: Result<Peaks, String>) {
        let slot = match peaks {
            Ok(peaks) => Slot::Ready(Arc::new(peaks)),
            Err(error) => {
                tracing::warn!("волна звука не прочитана: {error}");
                Slot::Failed
            }
        };
        match self.slots.iter_mut().find(|(s, _)| *s == source) {
            Some((_, old)) => *old = slot,
            None => self.slots.push((source, slot)),
        }
    }

    /// The peaks of `source`, once they came.
    pub fn peaks(&self, source: &Source) -> Option<&Peaks> {
        self.slots.iter().find_map(|(s, slot)| match slot {
            Slot::Ready(peaks) if s == source => Some(peaks.as_ref()),
            _ => None,
        })
    }

    /// The thread answered since the last call: the editor window repaints.
    pub fn take_news(&self) -> bool {
        self.worker
            .as_ref()
            .is_some_and(|w| w.news.swap(false, Ordering::AcqRel))
    }
}

/// The peaks of `source` in the project `dir`, from its cache when it has them.
pub(super) fn load(dir: &ProjectDir, source: &Source) -> Result<Peaks, String> {
    let track = match source {
        Source::Track { media, name } => tracks(dir, media)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|t| t.name == *name)
            .ok_or_else(|| format!("у {} нет дорожки {name}", media.path))?,
        Source::File(path) => {
            let file = path
                .split('/')
                .fold(dir.root().to_path_buf(), |p, part| p.join(part));
            file_track(&file).map_err(|e| e.to_string())?
        }
    };
    cached_peaks(dir, &track).map_err(|e| e.to_string())
}
