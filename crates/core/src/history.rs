//! Undo/redo. Every applied batch of commands is stored as a [`Change`]: the parts of the project
//! it touched (clips and/or individual effect states) before and after. Undo restores "before",
//! redo restores "after" — no hand-written inverse commands, so undo cannot be wrong.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::command::{Command, CommandError};
use crate::id::IdCounter;
use crate::project::{Clip, ClipKind, Project};

/// The parts of a project a change touched.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Parts {
    /// Main track and its id counter, if the change touched clips.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clips: Option<(Vec<Clip>, IdCounter<ClipKind>)>,
    /// Effect id → its state (`None` = no state, i.e. default).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub effects: BTreeMap<String, Option<serde_json::Value>>,
}

/// One undoable step: a single command or a whole batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    /// Command name, or `"apply"` for a batch plan.
    pub name: String,
    /// One-line summary of what changed.
    pub summary: String,
    pub before: Parts,
    pub after: Parts,
}

impl Parts {
    fn restore(&self, project: &mut Project) {
        if let Some((clips, counter)) = &self.clips {
            project.clips.clone_from(clips);
            project.clip_ids = *counter;
        }
        for (id, state) in &self.effects {
            match state {
                Some(value) => project.effects.insert(id.clone(), value.clone()),
                None => project.effects.remove(id),
            };
        }
    }
}

/// The parts that differ between `before` and `after`.
fn diff(before: &Project, after: &Project) -> (Parts, Parts) {
    let mut old = Parts::default();
    let mut new = Parts::default();
    if before.clips != after.clips || before.clip_ids != after.clip_ids {
        old.clips = Some((before.clips.clone(), before.clip_ids));
        new.clips = Some((after.clips.clone(), after.clip_ids));
    }
    let ids: std::collections::BTreeSet<&String> =
        before.effects.keys().chain(after.effects.keys()).collect();
    for id in ids {
        let (a, b) = (before.effects.get(id), after.effects.get(id));
        if a != b {
            old.effects.insert(id.clone(), a.cloned());
            new.effects.insert(id.clone(), b.cloned());
        }
    }
    (old, new)
}

/// Undo and redo stacks. `limit` caps how many steps are kept.
#[derive(Debug, Clone)]
pub struct History {
    undo: Vec<Change>,
    redo: Vec<Change>,
    limit: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(1),
        }
    }

    /// Restores stacks loaded from disk: `undo` oldest first, `redo` with the next step to redo last.
    pub fn from_stacks(undo: Vec<Change>, redo: Vec<Change>, limit: usize) -> Self {
        let mut history = Self::new(limit);
        history.undo = undo;
        history.redo = redo;
        history.trim();
        history
    }

    pub fn apply(
        &mut self,
        project: &mut Project,
        command: &dyn Command,
    ) -> Result<String, CommandError> {
        self.apply_batch(project, command.name(), &[command])
    }

    /// Applies all commands as one step, all or nothing. Returns the joined summary.
    /// A batch that changes nothing is not recorded.
    pub fn apply_batch(
        &mut self,
        project: &mut Project,
        name: &str,
        commands: &[&dyn Command],
    ) -> Result<String, CommandError> {
        let mut draft = project.clone();
        let mut summaries = Vec::new();
        for command in commands {
            summaries.push(command.apply(&mut draft)?);
            let problems = draft.problems();
            if !problems.is_empty() {
                return Err(CommandError::new(format!(
                    "внутренняя ошибка команды «{}»: она нарушила целостность проекта ({}). Изменения отменены; это баг, сообщи о нём",
                    command.name(),
                    problems.join("; ")
                )));
            }
        }
        let summary = summaries.join("; ");
        let (before, after) = diff(project, &draft);
        if before == after {
            return Ok(summary);
        }
        *project = draft;
        self.undo.push(Change {
            name: name.to_owned(),
            summary: summary.clone(),
            before,
            after,
        });
        self.redo.clear();
        self.trim();
        Ok(summary)
    }

    /// Undoes the last step; returns what was undone.
    pub fn undo(&mut self, project: &mut Project) -> Option<&Change> {
        let change = self.undo.pop()?;
        change.before.restore(project);
        self.redo.push(change);
        self.redo.last()
    }

    /// Redoes the last undone step; returns what was redone.
    pub fn redo(&mut self, project: &mut Project) -> Option<&Change> {
        let change = self.redo.pop()?;
        change.after.restore(project);
        self.undo.push(change);
        self.undo.last()
    }

    /// Undoable steps, oldest first.
    pub fn changes(&self) -> &[Change] {
        &self.undo
    }

    /// Undone steps that can be redone; the next one to redo is last.
    pub fn redo_changes(&self) -> &[Change] {
        &self.redo
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn trim(&mut self) {
        let excess = self.undo.len().saturating_sub(self.limit);
        self.undo.drain(..excess);
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
