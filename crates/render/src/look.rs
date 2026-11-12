//! The seam of an effect's picture (decision 015, docs/ARCHITECTURE.md «Картинка эффекта»): an
//! effect prepares itself once per project ([`Prepare`], what is heavy: states, telemetry, text
//! layout) and then puts its layers into the scene of each output moment ([`Look`]), a pure
//! function. `dc_ops::Evaluator` holds a list of them and calls them in its order; it knows no
//! effect's types.

use std::fmt::Debug;

use dc_core::{CursorTracks, KeyTracks, OutputTime, PixelSize, Project, StateError, TimeMap};

use crate::{RenderError, Scene, TextMeasure};

/// What an effect may prepare from: read once per project.
pub struct Inputs<'a> {
    pub project: &'a Project,
    pub map: &'a TimeMap,
    /// The output frame, px: the canvas after the crop.
    pub canvas: PixelSize,
    /// Mouse telemetry of the recordings on the track; the caller reads it from the project folder.
    pub cursors: &'a CursorTracks,
    /// Key telemetry of the recordings on the track.
    pub keys: &'a KeyTracks,
    /// The fonts the frames are drawn with: texts are measured and broken into lines here
    /// (decision 016).
    pub measure: &'a mut TextMeasure,
}

/// The output moment being built: the scene so far and what earlier effects left for later ones.
pub struct Frame<'a> {
    pub project: &'a Project,
    pub map: &'a TimeMap,
    pub t: OutputTime,
    pub scene: &'a mut Scene,
    /// Where the cursor points at `t`: the cursor sets it, the callout hangs on it. `None`
    /// outside the video or without telemetry.
    pub pointer: Option<Pointer>,
}

/// The cursor point at an output moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    /// Px of the canvas, before the camera.
    pub tip: [f64; 2],
    /// Visible arrow height without the click bounce, px of the canvas (v1 `H`).
    pub arrow: f64,
    /// Canvas px per ref-px: the clip's width on the canvas over 1920.
    pub k: f64,
}

/// An effect prepared for a project: it draws any output moment.
pub trait Look: Debug + Send + Sync {
    /// Puts the effect's layers of `frame.t` into `frame.scene`.
    fn draw(&self, frame: &mut Frame<'_>);
}

/// An effect's entry in the list of pictures: it prepares the effect for a project.
pub trait Prepare: Sync {
    fn prepare(&self, inputs: &mut Inputs<'_>) -> Result<Box<dyn Look>, LookError>;
}

/// An effect can not be prepared for the project.
#[derive(Debug, thiserror::Error)]
pub enum LookError {
    #[error(transparent)]
    State(#[from] StateError),

    #[error(transparent)]
    Text(#[from] RenderError),

    /// The effect's own reason; its text says what to do.
    #[error(transparent)]
    Effect(Box<dyn std::error::Error + Send + Sync>),
}
