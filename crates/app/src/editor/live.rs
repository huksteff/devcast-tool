//! `devcast` on the project of the editor (7.32, `dc_ops::live`): the main window answers the
//! requests of the app channel; the run's events become a step of the editor's history, the view
//! is rebuilt and the editor window repaints. The requests about the project as a whole (open,
//! close, the copy of `save --as`, rename) go the way of the window's own buttons (`switch.rs`).

use std::path::Path;
use std::time::Instant;

use dc_ops::live::{Answer, CHANGED, Changes, Request};

use super::{EditorWindow, Leave};
use crate::app::Shared;

impl Shared {
    /// Answers a `devcast` run: the editor's project, or «not open»; `devcast open` opens one.
    pub fn live(&mut self, request: Request) -> Answer {
        match request {
            Request::Edit { root } => return self.open_asked(&root),
            Request::Close { root, changes } => return self.close_asked(&root, changes),
            Request::Rename { root, name } => return self.rename_asked(&root, &name),
            Request::Copied {
                root,
                revision,
                copy,
            } => return self.copied_asked(&root, revision, &copy),
            Request::Settings { edit } => return self.configure(edit),
            Request::Current => {
                let root = self
                    .editor
                    .as_ref()
                    .map(|e| e.doc.dir().root().to_path_buf());
                return Answer::Current { root };
            }
            _ => {}
        }
        let Some(editor) = self.editor.as_mut() else {
            return Answer::NotOpen;
        };
        let (answer, problem) = editor.live(request, Instant::now());
        if let Some(problem) = problem {
            self.report(Err(problem));
        }
        answer
    }

    /// `devcast config` and `devcast hotkeys`: the edit goes through the one store, so the
    /// windows read it on their next frame and the global hotkeys are registered again in this
    /// pass of the main window (`App::hotkeys` runs after the calls).
    fn configure(&mut self, edit: dc_settings::Edit) -> Answer {
        let applied = match dc_settings::apply(self.store.get(), edit) {
            Ok(applied) => applied,
            Err(error) => return Answer::NotConfigured(error),
        };
        let settings = applied.settings.clone();
        if let Err(error) = self.store.update(|s| *s = settings) {
            return Answer::Refused(error.to_string());
        }
        if let Some(editor) = self.editor.as_mut() {
            editor.detached.wake_editor = true;
            editor.detached.wake_timeline = true;
        }
        self.ctx.request_repaint_of(eframe::egui::ViewportId::ROOT);
        Answer::Configured(Box::new(applied))
    }

    /// The editor of the project in `root`, if it is the open one.
    fn editor_of(&mut self, root: &Path) -> Option<&mut EditorWindow> {
        self.editor
            .as_mut()
            .filter(|e| same_folder(e.doc.dir().root(), root))
    }

    /// `devcast close`: unsaved changes are saved or dropped as asked, then the window closes.
    fn close_asked(&mut self, root: &Path, changes: Option<Changes>) -> Answer {
        let Some(editor) = self.editor_of(root) else {
            let open = self
                .editor
                .as_ref()
                .map(|e| e.doc.dir().root().to_path_buf());
            return Answer::Elsewhere { open };
        };
        if editor.doc.unsaved() {
            let done = match changes {
                None => return Answer::Unsaved,
                Some(Changes::Save) => editor.doc.save(),
                Some(Changes::Discard) => editor.doc.discard(),
            };
            if let Err(error) = done {
                return Answer::Refused(error.to_string());
            }
        }
        self.leave_project(Leave::Close);
        Answer::Closed
    }

    /// `devcast save --as` made the copy of the state of `revision`: as after «Сохранить как…».
    fn copied_asked(&mut self, root: &Path, revision: u64, copy: &Path) -> Answer {
        let Some(editor) = self.editor_of(root) else {
            return Answer::NotOpen;
        };
        if editor.doc.revision() != revision {
            return Answer::Refused(CHANGED.to_owned());
        }
        match self.take_copy(copy) {
            Ok(()) => Answer::Opened,
            Err(error) => Answer::Refused(error),
        }
    }

    /// `devcast rename`: as renaming by the header's name.
    fn rename_asked(&mut self, root: &Path, name: &str) -> Answer {
        if self.editor_of(root).is_none() {
            return Answer::NotOpen;
        }
        match self.try_rename(name) {
            Ok(root) => Answer::Renamed { root },
            Err(error) => Answer::Refused(error),
        }
    }
}

impl EditorWindow {
    /// Answers a request of a `devcast` run about the project in `root`, at `now`. Returns the
    /// answer and why the view could not follow (the sound of playback), for a toast.
    pub fn live(&mut self, request: Request, now: Instant) -> (Answer, Option<String>) {
        let root = match &request {
            Request::Open { root }
            | Request::Write { root, .. }
            | Request::Save { root }
            | Request::Discard { root } => root,
            _ => return (Answer::NotOpen, None),
        };
        if !same_folder(root, self.doc.dir().root()) {
            return (Answer::NotOpen, None);
        }
        match request {
            Request::Open { .. } => match self.doc.flush() {
                Ok(()) => (
                    Answer::Open {
                        revision: self.doc.revision(),
                    },
                    None,
                ),
                Err(error) => (Answer::Refused(error.to_string()), None),
            },
            Request::Write {
                revision, events, ..
            } => match self.doc.take(revision, events, now) {
                Some(revision) => {
                    let problem = self.refresh(now);
                    self.detached.wake_editor = true;
                    self.detached.wake_timeline = true;
                    (Answer::Written { revision }, problem)
                }
                None => (Answer::Refused(CHANGED.to_owned()), None),
            },
            Request::Save { .. } => match self.doc.save() {
                Ok(()) => {
                    self.detached.wake_editor = true;
                    (Answer::Saved, None)
                }
                Err(error) => (Answer::Refused(error.to_string()), None),
            },
            Request::Discard { .. } => self.discard_asked(now),
            _ => (Answer::NotOpen, None),
        }
    }

    /// `devcast discard`: as «Отбросить» of the recovery question.
    fn discard_asked(&mut self, now: Instant) -> (Answer, Option<String>) {
        if !self.doc.unsaved() {
            return (Answer::Discarded { changed: false }, None);
        }
        if let Err(error) = self.doc.discard() {
            return (Answer::Refused(error.to_string()), None);
        }
        self.recovery = false;
        let problem = self.refresh(now);
        self.detached.wake_editor = true;
        self.detached.wake_timeline = true;
        (Answer::Discarded { changed: true }, problem)
    }
}

/// Both paths name one folder (`..`, case and separators aside).
pub(super) fn same_folder(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
