//! `history.jsonl`: undo/redo kept between runs. An append-only log: the first line is the
//! schema version of its snapshots `{"schema_version":1}`, then one event per line:
//! `{"do":{change}}`, `"undo"`, `"redo"`. Replaying it rebuilds the stacks. Snapshots hold only the
//! parts a change touched, and migrations work on a whole `project.json`, so a log of another
//! schema version is not migrated: history starts anew with a warning.
//!
//! The log is appended right before `session.json`/`project.json` is written, so it describes the
//! state on disk. Every [`Change`] stores absolute snapshots, which gives two recovery rules:
//! - a corrupt line in the middle: events before it are dropped, the rest still replays correctly;
//! - after replay the stacks must fit the loaded project (top undo `after` and top redo `before`
//!   `Parts::fits` the project); otherwise (crash between the two writes, «Не сохранять») history resets.

use std::io::Write;
use std::path::PathBuf;

use dc_core::history::Parts;
use dc_core::project::SCHEMA_VERSION;
use dc_core::{Change, History, Project};
use serde::{Deserialize, Serialize};

use crate::atomic;
use crate::dir::{HISTORY_FILE, ProjectDir};
use crate::error::{ProjectError, Result};

/// Larger logs drop their oldest steps on compaction.
pub const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Event {
    Do(Change),
    Undo,
    Redo,
}

/// The first line of the log.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    schema_version: u32,
}

#[derive(Debug)]
pub struct Loaded {
    pub history: History,
    /// Problems found while reading, for the log and the agent; loading still succeeded.
    pub warnings: Vec<String>,
    /// The file is much larger than the stacks it holds or had problems: call [`HistoryLog::rewrite`].
    pub needs_rewrite: bool,
}

#[derive(Debug, Clone)]
pub struct HistoryLog {
    path: PathBuf,
}

impl HistoryLog {
    pub fn new(dir: &ProjectDir) -> Self {
        Self {
            path: dir.file(HISTORY_FILE),
        }
    }

    pub fn append(&self, events: &[Event]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let mut text = String::new();
        for event in events {
            text.push_str(&json_line(event)?);
            text.push('\n');
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(ProjectError::io("открыть", &self.path))?;
        let empty = file
            .metadata()
            .map_err(ProjectError::io("прочитать", &self.path))?
            .len()
            == 0;
        if empty {
            text.insert_str(0, &format!("{}\n", header()?));
        }
        file.write_all(text.as_bytes())
            .and_then(|()| file.sync_data())
            .map_err(ProjectError::io("записать", &self.path))
    }

    /// Rebuilds undo/redo for `project` (the state just loaded from disk). Never fails on bad
    /// content, only on IO: a broken history must not stop the project from opening.
    pub fn load(&self, project: &Project, limit: usize) -> Result<Loaded> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(ProjectError::io("прочитать", &self.path)(e)),
        };
        let (mut undo, mut redo, mut warnings) = (Vec::new(), Vec::new(), Vec::new());
        let mut lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let version = lines
            .first()
            .and_then(|l| serde_json::from_str::<Header>(l).ok())
            .map(|h| h.schema_version);
        if version.is_some() {
            lines.remove(0);
        }
        if version != Some(SCHEMA_VERSION) && !lines.is_empty() {
            let found = version.map_or("без версии".to_owned(), |v| format!("версии {v}"));
            warnings.push(format!(
                "история записана схемой проекта {found}, а программа пишет версию {SCHEMA_VERSION}: её снимки не переносятся, история начата заново"
            ));
            lines.clear();
        }
        for (index, line) in lines.iter().enumerate() {
            match serde_json::from_str::<Event>(line) {
                Ok(event) => replay(&mut undo, &mut redo, event),
                Err(e) if index + 1 == lines.len() => {
                    warnings.push(format!(
                        "последняя запись истории оборвана ({e}), пропущена"
                    ));
                }
                Err(e) => {
                    warnings.push(format!(
                        "запись истории №{} повреждена ({e}): шаги до неё отброшены",
                        index + 1
                    ));
                    undo.clear();
                    redo.clear();
                }
            }
        }
        let fits = |parts: Option<&Parts>| parts.is_none_or(|p| p.fits(project));
        if !fits(undo.last().map(|c| &c.after)) || !fits(redo.last().map(|c| &c.before)) {
            warnings.push("история не совпадает с проектом на диске и начата заново".to_owned());
            undo.clear();
            redo.clear();
        }
        let history = History::from_stacks(undo, redo, limit);
        let kept = history.changes().len() + 2 * history.redo_changes().len();
        let needs_rewrite =
            !warnings.is_empty() || lines.len() > 2 * kept + 64 || text.len() > MAX_BYTES;
        Ok(Loaded {
            history,
            warnings,
            needs_rewrite,
        })
    }

    /// Compacts the log to exactly the current stacks, dropping the oldest steps over [`MAX_BYTES`].
    pub fn rewrite(&self, history: &History) -> Result<()> {
        let redo = history.redo_changes();
        let mut undo_lines = history
            .changes()
            .iter()
            .map(|c| json_line(&Event::Do(c.clone())))
            .collect::<Result<Vec<_>>>()?;
        // Redo steps are written as done and then undone, the next one to redo undone last.
        let redo_lines = redo
            .iter()
            .rev()
            .map(|c| json_line(&Event::Do(c.clone())))
            .collect::<Result<Vec<_>>>()?;
        let undo_marks = vec![json_line(&Event::Undo)?; redo.len()];
        let size = |lines: &[String]| lines.iter().map(|l| l.len() + 1).sum::<usize>();
        let fixed = size(&redo_lines) + size(&undo_marks);
        let mut total = fixed + size(&undo_lines);
        let mut drop = 0;
        while total > MAX_BYTES && drop < undo_lines.len() {
            total -= undo_lines[drop].len() + 1;
            drop += 1;
        }
        undo_lines.drain(..drop);
        let mut text = String::with_capacity(total);
        text.push_str(&header()?);
        text.push('\n');
        for line in undo_lines.iter().chain(&redo_lines).chain(&undo_marks) {
            text.push_str(line);
            text.push('\n');
        }
        atomic::write(&self.path, text.as_bytes())
    }
}

fn header() -> Result<String> {
    json_line(&Header {
        schema_version: SCHEMA_VERSION,
    })
}

fn json_line(value: &impl Serialize) -> Result<String> {
    serde_json::to_string(value).map_err(|e| ProjectError::Corrupt {
        path: PathBuf::from(HISTORY_FILE),
        message: e.to_string(),
    })
}

fn replay(undo: &mut Vec<Change>, redo: &mut Vec<Change>, event: Event) {
    match event {
        Event::Do(change) => {
            undo.push(change);
            redo.clear();
        }
        // An undo/redo of a step dropped earlier has nothing to move; snapshots stay correct.
        Event::Undo => redo.extend(undo.pop()),
        Event::Redo => undo.extend(redo.pop()),
    }
}

#[cfg(test)]
#[path = "history_log_tests.rs"]
mod tests;
