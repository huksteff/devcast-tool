//! A project opened by one CLI run. Without the editor the CLI is the only writer and edits the
//! working state: `session.json` when the editor left unsaved changes (they stay unsaved until
//! `devcast save`), otherwise `project.json`, so the change is saved right away.
//!
//! A recording whose process died (killed, crashed) is finished and added to the project the
//! first time the project is opened without a writer holding it (a running recording holds it).

use dc_capture::project::{Recovered, recover_recordings, unfinished_recordings};
use dc_core::{History, Project, SourceTime, history};
use dc_project::history_log::Event;
use dc_project::{HistoryLog, ProjectDir, ProjectError, ProjectLock};

use crate::autozoom;
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
    /// With `write`, holds the project lock until the workspace is dropped. Under `dry_run`
    /// nothing is written, killed recordings are only reported.
    pub fn open(dir: ProjectDir, write: bool, dry_run: bool) -> Result<Self, CliError> {
        let lock = match write {
            true => Some(ProjectLock::acquire(&dir, "devcast")?),
            false => None,
        };
        let recovered = recover(&dir, lock.is_some(), dry_run)?;
        let added = recovered.clips > 0;
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
        let mut warnings = recovered.lines;
        warnings.extend(loaded.warnings);
        let mut workspace = Self {
            dir,
            log,
            _lock: lock.or(recovered.lock),
            unsaved,
            project,
            history: loaded.history,
            warnings,
        };
        if added {
            workspace.auto_zooms();
        }
        Ok(workspace)
    }

    /// Auto zooms for the recovered recordings, as after `devcast record`; a failure is a
    /// warning: the recording itself is already in the project.
    fn auto_zooms(&mut self) {
        let line = match autozoom::after_recording(self) {
            Ok(Some(applied)) if applied.changed => {
                format!("автозумы восстановленной записи: {applied}")
            }
            Ok(_) => return,
            Err(e) => format!("автозумы восстановленной записи не поставлены: {e}"),
        };
        self.warnings.push(line);
    }

    pub fn dir(&self) -> &ProjectDir {
        &self.dir
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

/// What opening found of killed recordings.
#[derive(Default)]
struct Recovery {
    /// A warning line per recording.
    lines: Vec<String>,
    /// Recordings added as clips.
    clips: usize,
    /// The lock taken for the recovery when the caller holds none: kept until the auto zooms of
    /// the recovered recordings are written.
    lock: Option<ProjectLock>,
}

/// Finishes the recordings of `dir` whose process died and adds them to its working state.
/// Skipped while another process holds the project: that may be the recording itself.
fn recover(dir: &ProjectDir, locked: bool, dry_run: bool) -> Result<Recovery, CliError> {
    let failed = |e: dc_capture::CaptureError| CliError::Failed(e.to_string());
    let media = dir.media_dir();
    let folders = unfinished_recordings(&media).map_err(failed)?;
    if dry_run {
        let lines = folders
            .iter()
            .map(|f| {
                format!("есть прерванная запись media/{f}, будет восстановлена при открытии без --dry-run")
            })
            .collect();
        return Ok(Recovery {
            lines,
            ..Recovery::default()
        });
    }
    if folders.is_empty() {
        return Ok(Recovery::default());
    }
    let lock = match locked {
        true => None,
        false => match ProjectLock::acquire(dir, "devcast") {
            Ok(lock) => Some(lock),
            Err(ProjectError::Locked { .. }) => return Ok(Recovery::default()),
            Err(e) => return Err(e.into()),
        },
    };
    let session = dir.load_session()?;
    let unsaved = session.is_some();
    let mut project = match session {
        Some(project) => project,
        None => dir.load()?,
    };
    let recovered = recover_recordings(&media, &folders, &mut project).map_err(failed)?;
    match unsaved {
        true => dir.save_session(&project)?,
        false => dir.save(&project)?,
    }
    Ok(Recovery {
        lines: recovered.iter().map(line).collect(),
        clips: recovered
            .iter()
            .filter(|r| matches!(r, Recovered::Clip { .. }))
            .count(),
        lock,
    })
}

fn line(recovered: &Recovered) -> String {
    match recovered {
        Recovered::Clip {
            folder,
            clip,
            duration,
        } => format!(
            "восстановлена прерванная запись media/{folder}: клип {clip}, {}",
            SourceTime::ZERO + *duration
        ),
        Recovered::Empty { folder } => format!(
            "прерванная запись media/{folder} оборвалась до первого целого сегмента: восстанавливать нечего, её файлы удалены"
        ),
    }
}
