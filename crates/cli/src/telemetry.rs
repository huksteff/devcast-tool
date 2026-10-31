//! Input telemetry of the recordings on the track (`dc_capture::project::recorded`), placed on
//! the output for `devcast events`. Auto zooms read it in `dc_ops::autozoom`, the evaluator
//! through `dc_capture::project::telemetry_tracks`. Imported videos have no telemetry.

use std::path::Path;

use dc_capture::project;
use dc_core::project::Media;
use dc_core::{Clip, OutputTime, Project, SourceTime, TimeMap};

pub(crate) use dc_capture::project::{Recorded, fraction};

use crate::error::CliError;

/// Every recording on the track, once each, in clip order.
pub(crate) fn read<'a>(root: &Path, project: &'a Project) -> Result<Vec<Recorded<'a>>, CliError> {
    project::recorded(root, project).map_err(|e| CliError::Failed(e.to_string()))
}

/// Where source moment `at` of `media` plays: output time and the clip; `None` if cut out.
pub(crate) fn place<'p>(
    map: &TimeMap,
    project: &'p Project,
    media: &Media,
    at: SourceTime,
) -> Option<(OutputTime, &'p Clip)> {
    let (segment, output) = map.locate(&media.path, at)?;
    let clip = project.clips.iter().find(|c| c.id == segment.clip)?;
    Some((output, clip))
}
