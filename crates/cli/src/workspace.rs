//! A project opened by one CLI run. Without the editor the CLI is the only writer and edits the
//! working state: `session.json` when the editor left unsaved changes (they stay unsaved until
//! `devcast save`), otherwise `project.json`, so the change is saved right away.

use dc_core::{History, Project, history};
use dc_project::history_log::Event;
use dc_project::{HistoryLog, ProjectDir, ProjectLock};

use crate::error::CliError;

#[derive(Debug)]
pub struct Workspace {
    dir: ProjectDir,
    log: HistoryLog,
    _lock: Option<ProjectLock>,
    unsaved: bool,
    pub project: Project,
    pub history: History,
    /// Problems found in `history.jsonl`; the project still opened.
    pub warnings: Vec<String>,
}

impl Workspace {
    /// With `write`, holds the project lock until the workspace is dropped.
    pub fn open(dir: ProjectDir, write: bool) -> Result<Self, CliError> {
        let lock = match write {
            true => Some(ProjectLock::acquire(&dir, "devcast")?),
            false => None,
        };
        let session = dir.load_session()?;
        let unsaved = session.is_some();
        let project = match session {
            Some(project) => project,
            None => dir.load()?,
        };
        let log = HistoryLog::new(&dir);
        let loaded = log.load(&project, history::LIMIT)?;
        if write && loaded.needs_rewrite {
            log.rewrite(&loaded.history)?;
        }
        Ok(Self {
            dir,
            log,
            _lock: lock,
            unsaved,
            project,
            history: loaded.history,
            warnings: loaded.warnings,
        })
    }

    /// Folder name, e.g. `Урок 1.dcproj`.
    pub fn name(&self) -> &str {
        self.dir.name()
    }

    /// True when the editor left unsaved changes and this run edits them.
    pub fn unsaved(&self) -> bool {
        self.unsaved
    }

    /// Writes `events`, which have just happened to `history`, and the project after them.
    pub fn record(&self, events: &[Event]) -> Result<(), CliError> {
        self.log.append(events)?;
        match self.unsaved {
            true => self.dir.save_session(&self.project)?,
            false => self.dir.save(&self.project)?,
        }
        Ok(())
    }

    /// `devcast save`: the working state becomes `project.json`, `session.json` goes away.
    pub fn save(&mut self) -> Result<(), CliError> {
        self.dir.save(&self.project)?;
        self.unsaved = false;
        Ok(())
    }
}
