//! Applying steps to a project: all of them as one undo step, or a dry run on a copy.

use std::fmt;

use dc_core::{Command, Diff, History, OutputTime, Project, TimeDelta, TimeMap};

use crate::error::OpsError;
use crate::plan::{Step, build, step_name};
use crate::registry::Registry;

/// What a change did. Shown as one line: `зумы +3 z12..z14; длительность 3:12.000→2:58.000`.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    /// Merged diff of all steps.
    pub diff: Diff,
    /// False when the steps left the project as it was (nothing was recorded in history).
    pub changed: bool,
    /// Output duration before and after.
    pub duration: (TimeDelta, TimeDelta),
    pub dry_run: bool,
}

/// Applies `steps` as one step of history: all or nothing. With `dry_run`, `project` and
/// `history` stay untouched and the result shows what would happen.
pub fn apply(
    registry: &Registry,
    project: &mut Project,
    history: &mut History,
    steps: &[Step],
    dry_run: bool,
) -> Result<Applied, OpsError> {
    let commands = build(registry, steps)?;
    let commands: Vec<&dyn Command> = commands.iter().map(AsRef::as_ref).collect();
    apply_commands(project, history, step_name(steps), &commands, dry_run)
}

/// Applies ready `commands` as one step of history named `name`, like [`apply`]: for commands
/// that need files prepared first (an imported video is copied into the project before its
/// clip is added).
pub fn apply_commands(
    project: &mut Project,
    history: &mut History,
    name: &str,
    commands: &[&dyn Command],
    dry_run: bool,
) -> Result<Applied, OpsError> {
    let original = project.clone();
    let before = TimeMap::build(project).duration();
    let (mut draft, mut draft_history);
    let (target, target_history) = if dry_run {
        draft = original.clone();
        draft_history = history.clone();
        (&mut draft, &mut draft_history)
    } else {
        (project, history)
    };
    let diff = target_history
        .apply_batch(target, name, commands)
        .map_err(|e| OpsError::Failed(e.to_string()))?;
    let changed = *target != original;
    Ok(Applied {
        diff,
        changed,
        duration: (before, TimeMap::build(target).duration()),
        dry_run,
    })
}

/// Commands of one type as the list [`apply_commands`] takes.
pub fn as_commands<C: Command>(commands: &[C]) -> Vec<&dyn Command> {
    commands.iter().map(|c| c as &dyn Command).collect()
}

impl fmt::Display for Applied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.dry_run {
            f.write_str("dry-run, проект не изменён: ")?;
        }
        if !self.changed {
            return f.write_str("без изменений");
        }
        write!(f, "{}", self.diff)?;
        let (before, after) = self.duration;
        match duration_change(before, after) {
            Some(change) => write!(f, "; {change}"),
            None => Ok(()),
        }
    }
}

/// `длительность 3:12.000→2:58.000`, or nothing if the output length did not change.
pub fn duration_change(before: TimeDelta, after: TimeDelta) -> Option<String> {
    let time = |d: TimeDelta| OutputTime::from_micros(d.micros());
    (before != after).then(|| format!("длительность {}→{}", time(before), time(after)))
}
