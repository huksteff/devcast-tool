//! Undo/redo. Every applied batch of commands is stored as a [`Change`]: the parts of the project
//! it touched (clips and/or individual effect states) before and after. Undo restores "before",
//! redo restores "after" — no hand-written inverse commands, so undo cannot be wrong.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::command::{Command, CommandError};
use crate::diff::Diff;
use crate::effect::IDS_FIELD;
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
    /// The run this step belongs to (one «Сделать красиво» of the user's AI agent): consecutive
    /// steps of the same group are one undo step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

impl Change {
    /// `next` (a later step of the same state) folded into this one: the parts as they were
    /// before both, and as they are after both.
    pub fn join(&mut self, next: Change) {
        if self.before.clips.is_none() {
            self.before.clips = next.before.clips;
        }
        for (id, state) in next.before.effects {
            self.before.effects.entry(id).or_insert(state);
        }
        if next.after.clips.is_some() {
            self.after.clips = next.after.clips;
        }
        self.after.effects.extend(next.after.effects);
        if !next.summary.is_empty() && !self.summary.contains(&next.summary) {
            self.summary = format!("{}; {}", self.summary, next.summary);
        }
    }
}

impl Parts {
    /// Whether `project` holds these parts: restoring them would change nothing.
    pub fn fits(&self, project: &Project) -> bool {
        let mut restored = project.clone();
        self.restore(&mut restored);
        restored == *project
    }

    /// Puts these parts back; id counters keep the higher of now and the snapshot, so an id is
    /// never handed out twice.
    fn restore(&self, project: &mut Project) {
        if let Some((clips, counter)) = &self.clips {
            project.clips.clone_from(clips);
            project.clip_ids = project.clip_ids.max(*counter);
        }
        for (id, state) in &self.effects {
            let mut restored = state.clone().unwrap_or_else(|| Value::Object(Map::new()));
            let now = project.effects.get(id).and_then(|s| s.get(IDS_FIELD));
            if let (Some(now), Value::Object(fields)) = (now, &mut restored) {
                // A missing counter is a fresh one, at 1.
                let then = fields.get(IDS_FIELD).and_then(Value::as_u64).unwrap_or(1);
                if now.as_u64().is_some_and(|n| n > then) {
                    fields.insert(IDS_FIELD.to_owned(), now.clone());
                }
            }
            if restored.as_object().is_some_and(Map::is_empty) {
                project.effects.remove(id);
            } else {
                project.effects.insert(id.clone(), restored);
            }
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

/// How many undo steps a project keeps by default.
pub const LIMIT: usize = 1000;

/// Undo and redo stacks. `limit` caps how many steps are kept.
#[derive(Debug, Clone)]
pub struct History {
    undo: Vec<Change>,
    redo: Vec<Change>,
    limit: usize,
    /// The group new steps get ([`Self::group`]).
    group: Option<String>,
}

impl Default for History {
    fn default() -> Self {
        Self::new(LIMIT)
    }
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(1),
            group: None,
        }
    }

    /// The group of the steps applied from now on: consecutive steps of one group are one undo
    /// step (`DEVCAST_GROUP` of a `devcast` run the AI agent started).
    pub fn group(&mut self, group: Option<String>) {
        self.group = group;
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
    ) -> Result<Diff, CommandError> {
        self.apply_batch(project, command.name(), &[command])
    }

    /// Applies all commands as one step, all or nothing. Returns their merged diff.
    /// A batch that changes nothing is not recorded.
    pub fn apply_batch(
        &mut self,
        project: &mut Project,
        name: &str,
        commands: &[&dyn Command],
    ) -> Result<Diff, CommandError> {
        let mut draft = project.clone();
        let mut summary = Diff::default();
        // In a batch, errors name the failing step: «шаг 3 (zoom add): …».
        let step = |index: usize, error: CommandError| match commands.len() {
            1 => error,
            _ => CommandError::new(format!(
                "шаг {} ({}): {error}. Ни один шаг не применён",
                index + 1,
                commands[index].name()
            )),
        };
        for (index, command) in commands.iter().enumerate() {
            let before = draft.clone();
            let diff = command.apply(&mut draft).map_err(|e| step(index, e))?;
            if diff.is_empty() && draft != before {
                return Err(step(
                    index,
                    CommandError::new(format!(
                        "внутренняя ошибка команды «{}»: она изменила проект, но не описала изменение. Изменения отменены; это баг, сообщи о нём",
                        command.name()
                    )),
                ));
            }
            summary = summary.and(diff);
            let problems = draft.problems();
            if !problems.is_empty() {
                return Err(step(
                    index,
                    CommandError::new(format!(
                        "внутренняя ошибка команды «{}»: она нарушила целостность проекта ({}). Изменения отменены; это баг, сообщи о нём",
                        command.name(),
                        problems.join("; ")
                    )),
                ));
            }
        }
        let (before, after) = diff(project, &draft);
        if before == after {
            return Ok(summary);
        }
        *project = draft;
        self.record(Change {
            name: name.to_owned(),
            summary: summary.to_string(),
            before,
            after,
            group: self.group.clone(),
        });
        Ok(summary)
    }

    /// Takes a step another writer made on this same state (a `devcast` run while the editor
    /// holds the project): `project` gets its "after" parts, it becomes the step to undo, and
    /// redo is cleared, as if [`Self::apply_batch`] had made it here.
    pub fn push(&mut self, project: &mut Project, change: Change) {
        change.after.restore(project);
        self.record(change);
    }

    /// `change` becomes the step to undo: joined with the top one when both are of one group.
    fn record(&mut self, change: Change) {
        match self.undo.last_mut() {
            Some(top) if top.group.is_some() && top.group == change.group => top.join(change),
            _ => self.undo.push(change),
        }
        self.redo.clear();
        self.trim();
    }

    pub fn undo(&mut self, project: &mut Project) -> Option<&Change> {
        let change = self.undo.pop()?;
        change.before.restore(project);
        self.redo.push(change);
        self.redo.last()
    }

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
