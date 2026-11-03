//! «Создать субтитры» (7.17): the model of the settings is downloaded first when it is not on
//! disk (`models.rs`, the download of the settings window), then the speech is recognized in a
//! worker process (`recognition`), and its cues replace all cues as `CaptionsGen` — one undo
//! step, the same command `devcast captions gen` applies. «Отменить» kills the worker. The UI
//! never waits: the download and the reader thread wake the main window, the editor takes their
//! news in its frame (`receive`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Where the generation is: the type of the captions panel's facts, which the editor fills.
pub use dc_fx_captions::panel::Phase;
use dc_fx_captions::{CaptionsGen, NewCue};
use dc_ui_kit::strings::captions as text;
use eframe::egui;

use crate::downloads::Downloads;
use crate::models::{ModelState, Models};
use crate::recognition::job::{Job, Spawn, cli_worker};
use crate::recognition::{Message, Request, Speech};
use crate::settings::{Language, Listen, SpeechModel};

enum Running {
    Idle,
    /// Waits for the model, then starts the worker with the request.
    Downloading(Box<Request>),
    Recognizing {
        job: Job,
        done: Option<f32>,
    },
}

pub struct Generation {
    /// The model files and their downloads.
    pub models: Models,
    /// The model, language and tracks of the settings, taken each frame; the captions panel
    /// changes the language and tracks, the frame writes them back.
    pub model: SpeechModel,
    pub language: Language,
    pub listen: Listen,
    running: Running,
    spawn: Spawn,
    news: Arc<AtomicBool>,
}

impl std::fmt::Debug for Generation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Generation")
            .field("model", &self.model)
            .field("phase", &self.phase())
            .finish_non_exhaustive()
    }
}

impl Default for Generation {
    /// No model folder (nothing is downloaded or ready) and the app's own worker.
    fn default() -> Self {
        Self::new(Models::new(None, Downloads::default()), cli_worker())
    }
}

impl Generation {
    pub fn new(models: Models, spawn: Spawn) -> Self {
        Self {
            models,
            model: SpeechModel::default(),
            language: Language::default(),
            listen: Listen::default(),
            running: Running::Idle,
            spawn,
            news: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn phase(&self) -> Phase {
        match &self.running {
            Running::Idle => Phase::Idle,
            Running::Downloading(_) => match self.models.state(self.model) {
                ModelState::Downloading(done) => Phase::Downloading(done),
                ModelState::Missing | ModelState::Ready => Phase::Downloading(None),
            },
            Running::Recognizing { done, .. } => Phase::Recognizing(*done),
        }
    }

    /// The model of the settings is on disk.
    pub fn model_ready(&self) -> bool {
        self.models.state(self.model) == ModelState::Ready
    }

    /// Starts recognizing the project of `request` (the model and the choices are put in).
    pub fn start(&mut self, ctx: &egui::Context, mut request: Request) -> Result<(), String> {
        request.speech = Speech {
            model: self.model.model(),
            language: self.language.code(),
            tracks: self.listen.tracks(),
        };
        if self.model_ready() {
            return self.recognize(ctx, &request);
        }
        if !self.models.can_download() {
            return Err(text::NO_MODELS.to_owned());
        }
        self.models
            .download(self.waker(ctx), self.model)
            .map_err(|e| e.to_string())?;
        self.running = Running::Downloading(Box::new(request));
        Ok(())
    }

    fn recognize(&mut self, ctx: &egui::Context, request: &Request) -> Result<(), String> {
        let child = (self.spawn)().map_err(|e| text::failed_start(&e))?;
        let job = Job::start(child, request, self.waker(ctx))?;
        self.running = Running::Recognizing { job, done: None };
        Ok(())
    }

    /// Kills the worker or stops waiting for the model (its download goes on in the
    /// background and is kept).
    pub fn cancel(&mut self) {
        self.running = Running::Idle;
    }

    /// The news of the download and of the worker: the cues once they came, or why it failed.
    pub fn receive(&mut self, ctx: &egui::Context) -> Option<Result<Vec<NewCue>, String>> {
        let downloaded = self.models.poll();
        match std::mem::replace(&mut self.running, Running::Idle) {
            Running::Idle => downloaded.err().map(|e| Err(e.to_string())),
            Running::Downloading(request) => {
                if let Err(error) = downloaded {
                    return Some(Err(error.to_string()));
                }
                if !self.model_ready() {
                    self.running = Running::Downloading(request);
                    return None;
                }
                self.recognize(ctx, &request).err().map(Err)
            }
            Running::Recognizing { job, done } => {
                let messages = job.receive();
                self.running = Running::Recognizing { job, done };
                self.take(messages)
            }
        }
    }

    /// Takes the messages of the worker: the outcome ends the job.
    fn take(&mut self, messages: Vec<Message>) -> Option<Result<Vec<NewCue>, String>> {
        for message in messages {
            match message {
                Message::Progress(share) => {
                    if let Running::Recognizing { done, .. } = &mut self.running {
                        *done = Some(share);
                    }
                }
                Message::Cues(cues) => {
                    self.running = Running::Idle;
                    return Some(Ok(cues));
                }
                Message::Failed(error) => {
                    self.running = Running::Idle;
                    return Some(Err(error));
                }
            }
        }
        None
    }

    /// Waits until the worker's whole answer is read (tests): the next frame takes all of it.
    #[cfg(test)]
    pub fn settle(&mut self) {
        if let Running::Recognizing { job, .. } = &mut self.running {
            job.settle();
        }
    }

    /// Waits for the next message of the worker and takes it (tests).
    #[cfg(test)]
    pub fn next(&mut self) -> Option<Result<Vec<NewCue>, String>> {
        let Running::Recognizing { job, .. } = &self.running else {
            return None;
        };
        let message = job.next()?;
        self.take(vec![message])
    }

    /// The download or the worker said something since the last call: the editor repaints.
    pub fn take_news(&self) -> bool {
        self.news.swap(false, Ordering::AcqRel)
    }

    fn waker(&self, ctx: &egui::Context) -> impl Fn() + Send + 'static {
        let (ctx, news) = (ctx.clone(), Arc::clone(&self.news));
        move || {
            news.store(true, Ordering::Release);
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
    }
}

/// «Создать субтитры» on the project of `editor` as it is now, unsaved changes too.
pub fn start(ctx: &egui::Context, editor: &mut super::EditorWindow) -> Result<(), String> {
    let request = Request {
        dir: editor.doc.dir().root().to_path_buf(),
        project: editor.doc.project().clone(),
        speech: Speech {
            model: editor.captions.model.model(),
            language: None,
            tracks: Vec::new(),
        },
    };
    editor.captions.start(ctx, request)
}

/// Applies the cues that came as `captions gen` does, one undo step at `now`: whether the
/// project changed, or why there are no new cues.
pub fn receive(
    ctx: &egui::Context,
    editor: &mut super::EditorWindow,
    now: std::time::Instant,
) -> Option<Result<bool, String>> {
    let cues = match editor.captions.receive(ctx)? {
        Ok(cues) => cues,
        Err(error) => return Some(Err(error)),
    };
    let command = CaptionsGen { cues };
    let applied = editor
        .doc
        .apply_commands(CaptionsGen::NAME, &[&command], now)
        .map(|a| a.changed)
        .map_err(|e| e.to_string());
    Some(applied)
}
