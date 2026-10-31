//! What the controls and shortcuts (`keys.rs`) ask of the project and how it is done: registry
//! commands through `Document::apply` (the path of `devcast`), undo and redo.

use std::time::Instant;

use dc_ops::Step;
use eframe::egui::Ui;
use serde_json::{Map, Value};

use super::{EditorWindow, Way};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Travel(Way),
    /// A registry command as one undo step.
    Apply(Step),
}

impl Action {
    /// The command `op` with `args` (`{"show": true}` is `--show true`).
    pub fn op(op: &str, args: impl IntoIterator<Item = (&'static str, Value)>) -> Self {
        let args: Map<String, Value> = args
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect();
        Self::Apply(Step::new(op, args))
    }
}

/// Runs the queued actions; the view is rebuilt when the project changed. Returns the errors.
pub fn run(ui: &Ui, editor: &mut EditorWindow, now: Instant) -> Vec<String> {
    let mut errors = Vec::new();
    let mut changed = false;
    for action in std::mem::take(&mut editor.actions) {
        match action {
            Action::Travel(way) => changed |= editor.doc.travel(way, now).is_some(),
            Action::Apply(step) => match editor.doc.apply(&[step], now) {
                Ok(applied) => changed |= applied.changed,
                Err(error) => errors.push(error.to_string()),
            },
        }
    }
    if changed {
        // Commands keep effect states valid, so a rebuilt view finds no new problems.
        editor.refresh();
        ui.ctx().request_repaint();
    }
    errors
}
