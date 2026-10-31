//! What the controls and shortcuts (`keys.rs`) ask of the project and of playback, and how it is
//! done: registry commands through `Document::apply` (the path of `devcast`), undo and redo,
//! play, pause and jumps of the playhead.

use std::time::Instant;

use dc_ops::Step;
use eframe::egui::Ui;
use serde_json::{Map, Value};

use super::playback::seek;
use super::{EditorWindow, Way};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Travel(Way),
    /// A registry command as one undo step.
    Apply(Step),
    Transport(Transport),
}

/// Playback and the playhead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// Play from the playhead (from the start when it is at the end) or pause.
    PlayPause,
    /// Pause and go this many frames (< 0 back).
    Step(i64),
    /// To the next keyframe (`true`) or the previous one, or 5 s that way; playback goes on.
    Jump(bool),
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
    let mut moved = false;
    for action in std::mem::take(&mut editor.actions) {
        match action {
            Action::Travel(way) => changed |= editor.doc.travel(way, now).is_some(),
            Action::Apply(step) => match editor.doc.apply(&[step], now) {
                Ok(applied) => changed |= applied.changed,
                Err(error) => errors.push(error.to_string()),
            },
            Action::Transport(transport) => {
                errors.extend(transport_run(editor, transport, now));
                moved = true;
            }
        }
    }
    if changed {
        // Commands keep effect states valid, so a rebuilt view finds no new problems.
        errors.extend(editor.refresh(now));
    }
    if changed || moved {
        ui.ctx().request_repaint();
    }
    errors
}

/// Does `transport` at `now`; the message of a sound that cannot start.
fn transport_run(editor: &mut EditorWindow, transport: Transport, now: Instant) -> Option<String> {
    let end = editor.end();
    let playback = &mut editor.playback;
    let view = &mut editor.view;
    match transport {
        Transport::PlayPause if playback.is_playing() => {
            view.playhead = playback.pause(now);
            None
        }
        Transport::PlayPause => {
            if view.playhead >= end {
                view.playhead = dc_core::OutputTime::ZERO;
            }
            playback.play(view.playhead, now)
        }
        Transport::Step(frames) => {
            if playback.is_playing() {
                view.playhead = playback.pause(now);
            }
            view.playhead = seek::step(view.playhead, frames, editor.fps, end);
            None
        }
        Transport::Jump(forward) => {
            let playing = playback.is_playing();
            let at = if playing {
                playback.pause(now)
            } else {
                view.playhead
            };
            view.playhead = seek::jump(at, forward, &editor.tracks.keys, end);
            (playing && view.playhead < end)
                .then(|| playback.play(view.playhead, now))
                .flatten()
        }
    }
}
