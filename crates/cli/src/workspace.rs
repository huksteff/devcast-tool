//! A project opened by one CLI run. Without the editor the CLI is the only writer and edits the
//! working state: `session.json` when the editor left unsaved changes (they stay unsaved until
//! `devcast save`), otherwise `project.json`, so the change is saved right away. While the editor
//! holds the project, the run reads the working state the editor puts on disk for it and sends
//! its changes to the editor (`live.rs`, 7.32).
//!
//! A recording whose process died (killed, crashed) is finished and added to the project the
//! first time the project is opened without a writer holding it (a running recording holds it).

use dc_core::{History, Project};
use dc_project::history_log::Event;
use dc_project::{HistoryLog, ProjectDir, ProjectError, ProjectLock, Working};

use crate::error::CliError;
use crate::live::Editor;

#[derive(Debug)]
pub struct Workspace {
    dir: ProjectDir,
    writer: Writer,
    unsaved: bool,
    pub project: Project,
    pub history: History,
    /// Problems found in `history.jsonl`; the project still opened.
    pub warnings: Vec<String>,
}

/// Where the changes of the run go.
#[derive(Debug)]
enum Writer {
    /// To the files, under the lock this run holds (none when it only reads).
    Files {
        log: HistoryLog,
        _lock: Option<ProjectLock>,
    },
    /// To the editor that holds the project.
    Editor(Editor),
}

impl Workspace {
    /// With `write`, holds the project lock until the workspace is dropped. Under `dry_run`
    /// nothing is written, killed recordings are only reported. A project the editor holds is
    /// edited through it, asked on `channel` (`None` — the real app channel).
    pub fn open(
        dir: ProjectDir,
        write: bool,
        dry_run: bool,
        channel: Option<&str>,
    ) -> Result<Self, CliError> {
        if let Some(editor) = Editor::open(&dir, channel)? {
            let working = Working::load(&dir, false)?;
            return Ok(Self {
                dir,
                writer: Writer::Editor(editor),
                unsaved: working.unsaved,
                project: working.project,
                history: working.history,
                warnings: working.warnings,
            });
        }
        let lock = match write {
            true => Some(ProjectLock::acquire(&dir, "devcast")?),
            false => None,
        };
        let (mut warnings, taken) = recover(&dir, lock.as_ref(), dry_run)?;
        let lock = lock.or(taken);
        let working = Working::load(&dir, write)?;
        let log = HistoryLog::new(&dir);
        warnings.extend(working.warnings);
        Ok(Self {
            dir,
            writer: Writer::Files { log, _lock: lock },
            unsaved: working.unsaved,
            project: working.project,
            history: working.history,
            warnings,
        })
    }

    pub fn dir(&self) -> &ProjectDir {
        &self.dir
    }

    /// Folder name, e.g. `Урок 1.devcast`.
    pub fn name(&self) -> &str {
        self.dir.name()
    }

    /// True when the editor left unsaved changes and this run edits them.
    pub fn unsaved(&self) -> bool {
        self.unsaved
    }

    /// The editor that holds the project, when the run goes through it.
    pub(crate) fn editor(&self) -> Option<&Editor> {
        match &self.writer {
            Writer::Editor(editor) => Some(editor),
            Writer::Files { .. } => None,
        }
    }

    /// Writes `events`, which have just happened to `history`, and the project after them; or
    /// sends the events to the editor, which takes them as one step of its history.
    pub fn record(&mut self, events: &[Event]) -> Result<(), CliError> {
        match &mut self.writer {
            Writer::Files { log, .. } => {
                log.append(events)?;
                match self.unsaved {
                    true => self.dir.save_session(&self.project)?,
                    false => self.dir.save(&self.project)?,
                }
            }
            Writer::Editor(editor) => editor.write(events)?,
        }
        Ok(())
    }

    /// `devcast save`: the working state becomes `project.json`, `session.json` goes away.
    pub fn save(&mut self) -> Result<(), CliError> {
        match &self.writer {
            Writer::Files { .. } => self.dir.save(&self.project)?,
            Writer::Editor(editor) => editor.save()?,
        }
        self.unsaved = false;
        Ok(())
    }
}

/// Finishes the recordings of `dir` whose process died and adds them to its working state
/// (`dc_ops::recovery`). Returns the warning lines and the lock taken for it when the caller
/// holds none. Skipped while another process holds the project: that may be the recording itself.
fn recover(
    dir: &ProjectDir,
    held: Option<&ProjectLock>,
    dry_run: bool,
) -> Result<(Vec<String>, Option<ProjectLock>), CliError> {
    let folders = dc_ops::recovery::interrupted(dir)?;
    if dry_run {
        let lines = folders
            .iter()
            .map(|f| {
                format!("есть прерванная запись media/{f}, будет восстановлена при открытии без --dry-run")
            })
            .collect();
        return Ok((lines, None));
    }
    if folders.is_empty() {
        return Ok((Vec::new(), None));
    }
    if let Some(lock) = held {
        return Ok((dc_ops::recovery::recover(dir, lock)?, None));
    }
    match ProjectLock::acquire(dir, "devcast") {
        Ok(lock) => Ok((dc_ops::recovery::recover(dir, &lock)?, Some(lock))),
        Err(ProjectError::Locked { .. }) => Ok((Vec::new(), None)),
        Err(e) => Err(e.into()),
    }
}
