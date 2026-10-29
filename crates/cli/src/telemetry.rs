//! Input telemetry of the recordings on the track (`dc_capture::input::Telemetry`), placed on
//! the output: for `devcast events` and auto zooms. A recording's folder is the folder of its
//! video (`media/recording-N/video.mp4`); imported videos have no telemetry.

use std::path::Path;

use dc_capture::input::Telemetry;
use dc_capture::project::recording_dir;
use dc_core::project::{Media, MediaKind};
use dc_core::{Clip, OutputTime, Point, Project, SourceTime, TimeMap};

use crate::error::CliError;

/// One recording of the project and its telemetry.
pub(crate) struct Recorded<'a> {
    pub media: &'a Media,
    pub telemetry: Telemetry,
}

/// Every recording on the track, once each, in clip order.
pub(crate) fn read<'a>(root: &Path, project: &'a Project) -> Result<Vec<Recorded<'a>>, CliError> {
    let mut recorded: Vec<Recorded<'a>> = Vec::new();
    for clip in &project.clips {
        let media = &clip.media;
        if media.kind != MediaKind::Recording || recorded.iter().any(|r| r.media.path == media.path)
        {
            continue;
        }
        let telemetry = Telemetry::read(&recording_dir(root, media))
            .map_err(|e| CliError::Failed(e.to_string()))?;
        recorded.push(Recorded { media, telemetry });
    }
    Ok(recorded)
}

/// Pixel (`x`, `y`) of the recorded picture as fractions of the frame of `media`.
pub(crate) fn fraction(media: &Media, x: i32, y: i32) -> Point {
    Point {
        x: f64::from(x) / f64::from(media.size.width),
        y: f64::from(y) / f64::from(media.size.height),
    }
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
