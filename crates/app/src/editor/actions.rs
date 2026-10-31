//! What the controls ask of the project and how it is done: registry commands through
//! `Document::apply` (the path of `devcast`), undo and redo. Shortcuts go by the physical key, so
//! Ctrl+Z works in the Russian layout and any other.

use std::time::Instant;

use dc_ops::Step;
use eframe::egui::{self, Key, Modifiers, Ui};
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

/// Ctrl+Z undo, Ctrl+Y and Ctrl+Shift+Z redo, pressed this frame.
pub fn shortcut(ui: &Ui) -> Option<Action> {
    ui.input(|i| {
        i.events.iter().find_map(|event| match event {
            egui::Event::Key {
                key,
                physical_key,
                pressed: true,
                modifiers,
                ..
            } => travel(physical_key.unwrap_or(*key), *modifiers).map(Action::Travel),
            _ => None,
        })
    })
}

fn travel(key: Key, modifiers: Modifiers) -> Option<Way> {
    if !modifiers.command || modifiers.alt {
        return None;
    }
    match (key, modifiers.shift) {
        (Key::Z, false) => Some(Way::Undo),
        (Key::Z, true) | (Key::Y, false) => Some(Way::Redo),
        _ => None,
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
