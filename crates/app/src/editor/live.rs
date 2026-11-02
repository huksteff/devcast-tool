//! `devcast` on the project of the editor (7.32, `dc_ops::live`): the main window answers the
//! requests of the app channel; the run's events become a step of the editor's history, the view
//! is rebuilt and the editor window repaints.

use std::path::Path;
use std::time::Instant;

use dc_ops::live::{Answer, CHANGED, Request};

use super::EditorWindow;
use crate::app::Shared;

impl Shared {
    /// Answers a `devcast` run: the editor's project, or «not open»; `devcast open` opens one.
    pub fn live(&mut self, request: Request) -> Answer {
        if let Request::Edit { root } = request {
            return self.open_asked(&root);
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
}

impl EditorWindow {
    /// Answers a request of a `devcast` run about the project in `root`, at `now`. Returns the
    /// answer and why the view could not follow (the sound of playback), for a toast.
    pub fn live(&mut self, request: Request, now: Instant) -> (Answer, Option<String>) {
        let root = match &request {
            Request::Show | Request::Edit { .. } => return (Answer::NotOpen, None),
            Request::Open { root } | Request::Write { root, .. } | Request::Save { root } => root,
        };
        if !same_folder(root, self.doc.dir().root()) {
            return (Answer::NotOpen, None);
        }
        match request {
            Request::Show | Request::Edit { .. } => (Answer::NotOpen, None),
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
        }
    }
}

/// Both paths name one folder (`..`, case and separators aside).
pub(super) fn same_folder(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
