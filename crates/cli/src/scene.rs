//! What every rendered frame of a project needs before the first scene, the same for `export`,
//! `frame` and `sheet`: the telemetry of its recordings and the fonts the captions and text
//! annotations are laid out with, built-in and the project's own (`dc_ops::fonts`, decision 016). Pictures of image annotations are read by
//! `dc_encode::Sources` as the scenes show them.

use std::path::Path;

use dc_core::Project;
use dc_ops::Evaluator;
use dc_render::{Compositor, Gpu, TextMeasure};

use crate::error::CliError;

/// The evaluator of `project` whose folder is `root`.
pub(crate) fn evaluator<'p>(root: &Path, project: &'p Project) -> Result<Evaluator<'p>, CliError> {
    let (cursors, keys) = dc_capture::project::telemetry_tracks(root, project)
        .map_err(|e| CliError::Failed(e.to_string()))?;
    let mut measure = TextMeasure::new();
    dc_ops::fonts::load(root, |dir| measure.load_fonts(dir)).map_err(fonts)?;
    Evaluator::new(project, cursors, keys, &mut measure)
        .map_err(|e| CliError::Failed(e.to_string()))
}

/// A compositor that draws with the same fonts as [`evaluator`] measures.
pub(crate) fn compositor(gpu: &Gpu, root: &Path) -> Result<Compositor, CliError> {
    let mut compositor = Compositor::new(gpu);
    dc_ops::fonts::load(root, |dir| compositor.load_fonts(dir)).map_err(fonts)?;
    Ok(compositor)
}

fn fonts(error: dc_render::RenderError) -> CliError {
    CliError::Failed(error.to_string())
}
