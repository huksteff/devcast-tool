//! Applying steps to a project: all of them as one undo step, or a dry run on a copy.

use dc_core::{Command, History, Project, TimeDelta, TimeMap};

use crate::error::OpsError;
use crate::plan::{Step, build};
use crate::registry::Registry;

/// What a change did, for the one-line answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    /// Joined summaries of the steps, e.g. `"+1 zoom z3; cut 10.000–12.000"`.
    pub summary: String,
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
    let name = match steps {
        [single] => single.op.clone(),
        _ => "apply".to_owned(),
    };
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
    let summary = target_history
        .apply_batch(target, &name, &commands)
        .map_err(|e| OpsError(e.to_string()))?;
    let changed = *target != original;
    Ok(Applied {
        summary,
        changed,
        duration: (before, TimeMap::build(target).duration()),
        dry_run,
    })
}
