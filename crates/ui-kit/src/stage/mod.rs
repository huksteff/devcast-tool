//! The kit of editing on the preview (docs/ARCHITECTURE.md, «Дорожка эффекта»): what the
//! gestures of an effect over the output frame work with, so they live in the effect's crate and
//! the editor only hosts them. Screen ↔ canvas px through the frame and the camera ([`geom`]),
//! a turned box with its handles and knob, snapping with guides ([`snap`]), the drawing of a
//! picked box, handles and guides ([`paint`]).
//!
//! A gesture keeps its own state (the editor stores it while the button is held) and answers
//! each frame with an [`Answer`]: a draft the preview shows while the gesture goes on, then on
//! release the registry commands of one undo step, as `devcast` takes them.

pub mod geom;
pub mod paint;
pub mod snap;

#[cfg(test)]
mod geom_tests;

use dc_core::Step;

/// What a gesture on the preview asks of the editor this frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Answer {
    /// The pointer works on the effect's overlay: the editor gives it no other gesture.
    pub ours: bool,
    /// The project as these commands make it shows on the preview; the gesture goes on.
    pub draft: Option<Vec<Step>>,
    /// The gesture ended: its commands (none when nothing changed), one undo step.
    pub done: Option<Option<Vec<Step>>>,
}
