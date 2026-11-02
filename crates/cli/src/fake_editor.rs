//! A fake editor for the tests of `devcast` on a project the app holds: on its own channel it
//! holds the lock as the app does and keeps the project in memory the way `Document` does
//! (writes the working state on `Open`, takes events with `dc_ops::live::replay`, discards with
//! `Working::discard`).

use std::path::Path;
use std::thread::JoinHandle;

use dc_core::{History, Project};
use dc_ipc::Listener;
use dc_ops::live::{Answer, CHANGED, Changes, LOCK_OWNER, Request};
use dc_project::history_log::Event;
use dc_project::{HistoryLog, ProjectDir, ProjectLock, Working};
use dc_settings::Store;

use crate::test_util::{Fixture, defaults_file};
use crate::{Local, Outcome, run_with};

pub(crate) fn channel(case: &str) -> String {
    dc_ipc::socket_name(&format!("test-live-{case}-{}", std::process::id())).unwrap()
}

/// The editor holding `f`'s project until it gets `Show` (or closes, or renames the project);
/// then it gives back its state.
pub(crate) struct FakeEditor {
    pub channel: String,
    server: JoinHandle<(Project, History)>,
}

impl FakeEditor {
    pub(crate) fn start(f: &Fixture, case: &str) -> Self {
        let channel = channel(case);
        let listener = Listener::bind(&channel).unwrap();
        let lock = ProjectLock::acquire(&f.dir, LOCK_OWNER).unwrap();
        let dir = f.dir.clone();
        let settings = defaults_file(f._tmp.path()).with_file_name(dc_settings::FILE);
        let store = Store::load(&settings).unwrap();
        let server = std::thread::spawn(move || {
            let mut state = State::new(dir, lock, store);
            loop {
                let mut connection = listener.accept().unwrap();
                let (answer, done) = state.answer(connection.receive().unwrap());
                connection.send(&answer).unwrap();
                if done {
                    return (state.project, state.history);
                }
            }
        });
        Self { channel, server }
    }

    /// Its state after `Show`.
    pub(crate) fn stop(self) -> (Project, History) {
        let shown: Answer = dc_ipc::request(&self.channel, &Request::Show).unwrap();
        assert_eq!(shown, Answer::Shown);
        self.server.join().unwrap()
    }

    /// Its state after it closed or renamed the project by itself.
    pub(crate) fn gone(self) -> (Project, History) {
        self.server.join().unwrap()
    }
}

struct State {
    dir: ProjectDir,
    lock: Option<ProjectLock>,
    log: HistoryLog,
    project: Project,
    history: History,
    revision: u64,
    pending: Vec<Event>,
    unsaved: bool,
    /// The app settings, in the settings folder of the runs (`run`).
    store: Store,
    /// The app's recording and export queue.
    app: crate::fake_app::FakeApp,
}

impl State {
    fn new(dir: ProjectDir, lock: ProjectLock, store: Store) -> Self {
        let working = Working::load(&dir, true).unwrap();
        Self {
            log: HistoryLog::new(&dir),
            dir,
            lock: Some(lock),
            project: working.project,
            history: working.history,
            revision: 0,
            pending: Vec::new(),
            unsaved: working.unsaved,
            store,
            app: crate::fake_app::FakeApp::default(),
        }
    }

    fn ours(&self, root: &Path) -> bool {
        std::fs::canonicalize(root).ok() == std::fs::canonicalize(self.dir.root()).ok()
    }

    /// The answer, and whether the editor is gone after it.
    fn answer(&mut self, request: Request) -> (Answer, bool) {
        let answer = match request {
            Request::Show => return (Answer::Shown, true),
            Request::Open { root } => {
                assert!(self.ours(&root));
                self.log.append(&self.pending).unwrap();
                self.pending.clear();
                self.dir.save_session(&self.project).unwrap();
                Answer::Open {
                    revision: self.revision,
                }
            }
            Request::Write {
                revision, events, ..
            } if revision == self.revision => {
                self.pending.extend(events.iter().cloned());
                dc_ops::live::replay(&mut self.project, &mut self.history, events);
                self.revision += 1;
                self.unsaved = true;
                Answer::Written {
                    revision: self.revision,
                }
            }
            Request::Write { .. } => Answer::Refused(CHANGED.to_owned()),
            Request::Edit { root } if self.ours(&root) => Answer::Opened,
            Request::Edit { .. } => Answer::Asking,
            Request::Save { .. } => {
                self.save();
                Answer::Saved
            }
            Request::Discard { root } if self.ours(&root) => Answer::Discarded {
                changed: self.discard(),
            },
            other => return self.whole(other),
        };
        (answer, false)
    }

    /// The requests about the project as a whole (closing, the copy, renaming) and about the app
    /// (its settings, the open project).
    fn whole(&mut self, request: Request) -> (Answer, bool) {
        match request {
            Request::Close { root, .. } if !self.ours(&root) => {
                let open = Some(self.dir.root().to_path_buf());
                (Answer::Elsewhere { open }, false)
            }
            Request::Close { changes: None, .. } if self.unsaved => (Answer::Unsaved, false),
            Request::Close { changes, .. } => {
                match changes {
                    Some(Changes::Save) => self.save(),
                    Some(Changes::Discard) => {
                        self.discard();
                    }
                    None => {}
                }
                (Answer::Closed, true)
            }
            Request::Copied { revision, .. } if revision != self.revision => {
                (Answer::Refused(CHANGED.to_owned()), false)
            }
            Request::Copied { .. } => {
                self.discard();
                (Answer::Opened, false)
            }
            Request::Rename { name, .. } => {
                drop(self.lock.take());
                let lock = ProjectLock::acquire(&self.dir, LOCK_OWNER).unwrap();
                let root = self.dir.rename(lock, &name).unwrap().root().to_path_buf();
                (Answer::Renamed { root }, true)
            }
            Request::Settings { edit } => {
                let answer = match dc_settings::apply(self.store.get(), edit) {
                    Ok(applied) => {
                        let settings = applied.settings.clone();
                        self.store.update(|s| *s = settings).unwrap();
                        Answer::Configured(Box::new(applied))
                    }
                    Err(error) => Answer::NotConfigured(error),
                };
                (answer, false)
            }
            Request::Current => {
                let root = Some(self.dir.root().to_path_buf());
                (Answer::Current { root }, false)
            }
            Request::Quit => (Answer::Quit { recorded: None }, true),
            other => (self.app.answer(other, &self.dir, self.unsaved), false),
        }
    }

    fn save(&mut self) {
        self.log.append(&self.pending).unwrap();
        self.pending.clear();
        self.dir.save(&self.project).unwrap();
        self.unsaved = false;
    }

    /// Drops the working state; true when there was one.
    fn discard(&mut self) -> bool {
        if !self.unsaved {
            return false;
        }
        let lock = self.lock.as_ref().unwrap();
        self.project = Working::discard(&self.dir, lock).unwrap();
        self.history = History::default();
        self.pending.clear();
        self.unsaved = false;
        self.revision += 1;
        true
    }
}

/// `devcast <line>` in `f` with the app on `channel`.
pub(crate) fn run(f: &Fixture, line: &str, channel: &str) -> Outcome {
    run_in(f, &f.cwd, line, channel)
}

/// [`run`] with `cwd` as the current folder.
pub(crate) fn run_in(f: &Fixture, cwd: &std::path::Path, line: &str, channel: &str) -> Outcome {
    let defaults = defaults_file(f._tmp.path());
    let local = Local {
        settings: defaults.parent(),
        app_channel: Some(channel),
        record_channel: Some(crate::test_util::NO_RECORDING),
        export_channel: Some(crate::test_util::EXPORTS),
        ..Local::default()
    };
    let args = line.split(' ').map(str::to_owned).collect();
    run_with(args, cwd, &f.registry, local)
}
