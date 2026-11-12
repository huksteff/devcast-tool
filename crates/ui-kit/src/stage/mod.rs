//! The kit of editing on the preview (docs/ARCHITECTURE.md, «Дорожка эффекта»): what the
//! gestures of an effect over the output frame work with, so they live in the effect's crate and
//! the editor only hosts them. Screen ↔ canvas px through the frame and the camera ([`geom`]),
//! a turned box with its handles and knob, snapping with guides ([`snap`]), the drawing of a
//! picked box, handles and guides ([`paint`]).
//!
//! An effect's gesture is a [`Stage`]: an entry of the editor's table. It keeps its own state
//! ([`Held`], the editor stores it while the button is held) and answers each frame with an
//! [`Answer`]: a draft the preview shows while the gesture goes on, then on release the registry
//! commands of one undo step, as `devcast` takes them.

pub mod geom;
mod gesture;
pub mod paint;
pub mod snap;

#[cfg(test)]
mod geom_tests;

pub use gesture::{Choice, Cx, Gesture, Held, Kept, Pick, Picked, Shown, Stage};

use dc_core::Step;

/// What a gesture on the preview asks of the editor this frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Answer {
    /// The pointer works on the effect's overlay: the editor gives it to no later stage.
    pub ours: bool,
    /// What the press picks.
    pub pick: Option<Pick>,
    /// The project as these commands make it shows on the preview; the gesture goes on.
    pub draft: Option<Vec<Step>>,
    /// The gesture ended: its commands (none when nothing changed), one undo step. The editor
    /// drops the held state.
    pub done: Option<Option<Vec<Step>>>,
}
