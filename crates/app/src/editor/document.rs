//! The project open in the editor for editing: the editor is its one writer (project lock) and
//! changes it only with registry commands through `dc_ops`, like `devcast`, so the same command
//! gives the same project in both.
//!
//! Saving follows dc-project: every change is unsaved until Ctrl+S (7.28); the working state goes
//! to `session.json` by `Autosave` (after a pause in editing), its history events to
//! `history.jsonl` right before it, so the log always describes a state on disk.

use std::time::{Duration, Instant};

use dc_core::{Change, History, Project};
use dc_ops::{Applied, OpsError, Registry, Step};
use dc_project::history_log::Event;
use dc_project::{Autosave, HistoryLog, ProjectDir, ProjectError, ProjectLock, Working};

/// Owner name in `lock-owner.json` while the app holds a project.
pub use dc_ops::live::LOCK_OWNER;

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Ops(#[from] OpsError),
}

#[derive(Debug)]
pub struct Document {
    dir: ProjectDir,
    _lock: ProjectLock,
    log: HistoryLog,
    registry: Registry,
    project: Project,
    history: History,
    autosave: Autosave,
    /// History events not in `history.jsonl` yet: written with the next `session.json`.
    pending: Vec<Event>,
    /// Grows with every change: a `devcast` run writes only over the state it read (7.32).
    revision: u64,
}

/// Which way undo and redo move through history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    Undo,
    Redo,
}

impl Document {
    /// Locks `dir` and loads its working state as `devcast` does. Also returns the problems
    /// found in its history (the project still opened).
    pub fn open(dir: ProjectDir) -> Result<(Self, Vec<String>), DocumentError> {
        let lock = ProjectLock::acquire(&dir, LOCK_OWNER)?;
        let working = Working::load(&dir, true)?;
        let document = Self {
            log: HistoryLog::new(&dir),
            dir,
            _lock: lock,
            registry: Registry::devcast()?,
            project: working.project,
            history: working.history,
            autosave: Autosave::default().restored(working.unsaved),
            pending: Vec::new(),
            revision: 0,
        };
        Ok((document, working.warnings))
    }

    pub fn dir(&self) -> &ProjectDir {
        &self.dir
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    /// The working state differs from `project.json`: the blue dot of the header.
    pub fn unsaved(&self) -> bool {
        self.autosave.has_unsaved()
    }

    /// Registry commands as one undo step, exactly as `devcast` applies them.
    pub fn apply(&mut self, steps: &[Step], now: Instant) -> Result<Applied, OpsError> {
        let applied = dc_ops::apply(
            &self.registry,
            &mut self.project,
            &mut self.history,
            steps,
            false,
        )?;
        Ok(self.record(applied, now))
    }

    /// The project as `steps` would make it, without changing it or its history: what the
    /// preview shows while a drag on it goes on (7.22).
    pub fn draft(&self, steps: &[Step]) -> Result<Project, OpsError> {
        let mut project = self.project.clone();
        let mut history = History::default();
        dc_ops::apply(&self.registry, &mut project, &mut history, steps, false)?;
        Ok(project)
    }

    /// Ready commands (not of the registry: `audio add` after its file is copied) as one undo
    /// step named `name`, as `devcast` applies them.
    pub fn apply_commands(
        &mut self,
        name: &str,
        commands: &[&dyn dc_core::Command],
        now: Instant,
    ) -> Result<Applied, OpsError> {
        let applied =
            dc_ops::apply_commands(&mut self.project, &mut self.history, name, commands, false)?;
        Ok(self.record(applied, now))
    }

    /// Queues the history event of a change for the session.
    fn record(&mut self, applied: Applied, now: Instant) -> Applied {
        if applied.changed
            && let Some(change) = self.history.changes().last()
        {
            self.pending.push(Event::Do(change.clone()));
            self.autosave.changed(now);
            self.revision += 1;
        }
        applied
    }

    /// The working state as a revision: what a `devcast` run reads after [`Self::flush`].
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Takes the history events a `devcast` run made of the state of `revision`, as one more
    /// step of this history (`dc_ops::live`); refused when the state has changed since. Returns
    /// the new revision.
    pub fn take(&mut self, revision: u64, events: Vec<Event>, now: Instant) -> Option<u64> {
        if revision != self.revision {
            return None;
        }
        self.pending.extend(events.iter().cloned());
        dc_ops::live::replay(&mut self.project, &mut self.history, events);
        self.autosave.changed(now);
        self.revision += 1;
        Some(self.revision)
    }

    /// Ctrl+S: the history events, then `project.json`; `session.json` goes away.
    pub fn save(&mut self) -> Result<(), ProjectError> {
        self.log.append(&self.pending)?;
        self.pending.clear();
        self.dir.save(&self.project)?;
        self.autosave.saved();
        Ok(())
    }

    /// One step back or forward; the step taken, `None` when there is none.
    pub fn travel(&mut self, way: Way, now: Instant) -> Option<Change> {
        let (change, event) = match way {
            Way::Undo => (self.history.undo(&mut self.project).cloned(), Event::Undo),
            Way::Redo => (self.history.redo(&mut self.project).cloned(), Event::Redo),
        };
        if change.is_some() {
            self.pending.push(event);
            self.autosave.changed(now);
            self.revision += 1;
        }
        change
    }

    /// The step undo (or redo) would take now.
    pub fn next(&self, way: Way) -> Option<&Change> {
        match way {
            Way::Undo => self.history.changes().last(),
            Way::Redo => self.history.redo_changes().last(),
        }
    }

    /// Writes the session when autosave says it is time; how long until the next one is due.
    pub fn autosave(&mut self, now: Instant) -> Result<Option<Duration>, ProjectError> {
        if self.autosave.due(now)
            && let Err(error) = self.write()
        {
            self.autosave.failed(now);
            return Err(error);
        }
        Ok(self.autosave.wait(now))
    }

    /// Writes what is not on disk yet (closing the editor, leaving the app).
    pub fn flush(&mut self) -> Result<(), ProjectError> {
        if !self.autosave.has_unwritten() {
            return Ok(());
        }
        self.write()
    }

    fn write(&mut self) -> Result<(), ProjectError> {
        self.log.append(&self.pending)?;
        self.pending.clear();
        self.dir.save_session(&self.project)?;
        self.autosave.session_written();
        Ok(())
    }
}
