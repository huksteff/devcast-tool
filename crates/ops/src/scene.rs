//! What every rendered frame of a project needs before the first scene, the same for the export
//! (`devcast export`, the editor's «Экспорт»), `frame` and `sheet`: the telemetry of its
//! recordings and the fonts the captions and text annotations are laid out with, built-in and
//! the project's own (`fonts`, decision 016). Pictures of image annotations are read by
//! `dc_encode::Sources` as the scenes show them.

use std::path::Path;

use dc_core::Project;
use dc_render::{Compositor, Gpu, RenderError, TextMeasure};

use crate::evaluate::{EvaluateError, Evaluator};

#[derive(Debug, thiserror::Error)]
pub enum SceneError {
    #[error(transparent)]
    Telemetry(#[from] dc_capture::CaptureError),
    #[error(transparent)]
    Fonts(#[from] RenderError),
    #[error(transparent)]
    Evaluate(#[from] EvaluateError),
}

/// The evaluator of `project` whose folder is `root`.
pub fn evaluator<'p>(root: &Path, project: &'p Project) -> Result<Evaluator<'p>, SceneError> {
    let (cursors, keys) = dc_capture::project::telemetry_tracks(root, project)?;
    let mut measure = TextMeasure::new();
    crate::fonts::load(root, |dir| measure.load_fonts(dir))?;
    Ok(Evaluator::new(project, cursors, keys, &mut measure)?)
}

/// A compositor that draws with the same fonts as [`evaluator`] measures.
pub fn compositor(gpu: &Gpu, root: &Path) -> Result<Compositor, SceneError> {
    let mut compositor = Compositor::new(gpu);
    crate::fonts::load(root, |dir| compositor.load_fonts(dir))?;
    Ok(compositor)
}
