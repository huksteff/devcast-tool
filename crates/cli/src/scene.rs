//! What every rendered frame of a project needs before the first scene, the same for `export`,
//! `frame` and `sheet`: the telemetry of its recordings and the built-in fonts the captions and
//! text annotations are laid out with (decision 016). Pictures of image annotations are read by
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
    measure.load_builtin_fonts().map_err(fonts)?;
    Evaluator::new(project, cursors, keys, &mut measure)
        .map_err(|e| CliError::Failed(e.to_string()))
}

/// A compositor that draws with the same fonts as [`evaluator`] measures.
pub(crate) fn compositor(gpu: &Gpu) -> Result<Compositor, CliError> {
    let mut compositor = Compositor::new(gpu);
    compositor.load_builtin_fonts().map_err(fonts)?;
    Ok(compositor)
}

fn fonts(error: dc_render::RenderError) -> CliError {
    CliError::Failed(error.to_string())
}
