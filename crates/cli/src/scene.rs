//! What every rendered frame of a project needs before the first scene (`dc_ops::scene`, the
//! same for `export`, `frame`, `sheet` and the editor), as errors of the CLI.

use std::path::Path;

use dc_core::Project;
use dc_ops::Evaluator;
use dc_render::{Compositor, Gpu};

use crate::error::CliError;

/// The evaluator of `project` whose folder is `root`.
pub(crate) fn evaluator<'p>(root: &Path, project: &'p Project) -> Result<Evaluator<'p>, CliError> {
    dc_ops::scene::evaluator(root, project).map_err(|e| CliError::Failed(e.to_string()))
}

/// A compositor that draws with the same fonts as [`evaluator`] measures.
pub(crate) fn compositor(gpu: &Gpu, root: &Path) -> Result<Compositor, CliError> {
    dc_ops::scene::compositor(gpu, root).map_err(|e| CliError::Failed(e.to_string()))
}
