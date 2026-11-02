//! The working state a writer (the editor, a CLI run) opens: `session.json` when unsaved changes
//! are left, otherwise `project.json`, and undo/redo from `history.jsonl` checked against it.

use dc_core::{History, Project, history};

use crate::dir::ProjectDir;
use crate::error::Result;
use crate::history_log::HistoryLog;
use crate::lock::ProjectLock;

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

    /// «Не сохранять», «Отбросить», `devcast discard`: back to `project.json`, which is returned.
    /// The working state goes away and so does the history, whose steps lead to it. `_lock`
    /// proves the caller is the one writer.
    pub fn discard(dir: &ProjectDir, _lock: &ProjectLock) -> Result<Project> {
        let project = dir.load()?;
        HistoryLog::new(dir).rewrite(&History::default())?;
        dir.discard_session()?;
        Ok(project)
    }
}

#[cfg(test)]
mod tests {
    use dc_core::{Command, CommandError, Diff};

    use super::*;

    #[derive(Debug)]
    struct Mark;
    impl Command for Mark {
        fn name(&self) -> &'static str {
            "test mark"
        }
        fn apply(&self, project: &mut Project) -> std::result::Result<Diff, CommandError> {
            project.effects.insert("test".to_owned(), 1.into());
            Ok(Diff::note("test=1".to_owned()))
        }
    }

    #[test]
    fn discard_goes_back_to_the_saved_project_without_history() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = ProjectDir::create(tmp.path().join("Урок с пробелами.devcast")).unwrap();
        let saved = Project::default();
        dir.save(&saved).unwrap();
        let (mut working, mut history) = (saved.clone(), History::default());
        history.apply(&mut working, &Mark).unwrap();
        HistoryLog::new(&dir).rewrite(&history).unwrap();
        dir.save_session(&working).unwrap();
        let lock = ProjectLock::acquire(&dir, "test").unwrap();
        assert_eq!(Working::discard(&dir, &lock).unwrap(), saved);
        let loaded = Working::load(&dir, false).unwrap();
        assert!(!loaded.unsaved);
        assert_eq!(loaded.project, saved);
        assert!(!loaded.history.can_undo());
    }
}
