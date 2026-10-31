//! The working state a writer (the editor, a CLI run) opens: `session.json` when unsaved changes
//! are left, otherwise `project.json`, and undo/redo from `history.jsonl` checked against it.

use dc_core::{History, Project, history};

use crate::dir::ProjectDir;
use crate::error::Result;
use crate::history_log::HistoryLog;

#[derive(Debug)]
pub struct Working {
    pub project: Project,
    pub history: History,
    /// It came from `session.json`: there are changes not saved to `project.json`.
    pub unsaved: bool,
    /// Problems found in `history.jsonl`; the project still opened.
    pub warnings: Vec<String>,
}

impl Working {
    /// With `write` (the caller holds the project lock) a damaged or bloated log is compacted.
    pub fn load(dir: &ProjectDir, write: bool) -> Result<Self> {
        let session = dir.load_session()?;
        let unsaved = session.is_some();
        let project = match session {
            Some(project) => project,
            None => dir.load()?,
        };
        let log = HistoryLog::new(dir);
        let loaded = log.load(&project, history::LIMIT)?;
        if write && loaded.needs_rewrite {
            log.rewrite(&loaded.history)?;
        }
        Ok(Self {
            project,
            history: loaded.history,
            unsaved,
            warnings: loaded.warnings,
        })
    }
}
