//! Where the cursor callout hangs (docs/porting/clicks-callout.md §11): the drawn cursor when it
//! is shown, the recorded pointer when it is not; `dc_ops::Evaluator` hands it to the callout.

use dc_core::{OutputTime, PixelSize, Project, TimeMap};

use crate::pose::pose_at;
use crate::prepared::{Prepared, REFERENCE_WIDTH};

/// The cursor point at an output moment, px of the frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    pub tip: [f64; 2],
    /// Height of the arrow without the click bounce (v1 `H`).
    pub arrow: f64,
    /// Frame px per ref-px: the clip's width on the frame over 1920.
    pub k: f64,
}

/// The pointer at output moment `t`; `None` outside the video, over a clip without telemetry or
/// outside the clip's crop. It does not depend on the cursor picture: a hidden system cursor
/// (typing) keeps the callout where the pointer is.
pub fn pointer(
    prepared: &Prepared,
    project: &Project,
    map: &TimeMap,
    t: OutputTime,
    frame: PixelSize,
) -> Option<Pointer> {
    let place = prepared.place(project, map, t, frame)?;
    let k = f64::from(place.content.w) / REFERENCE_WIDTH;
    let path = &place.recording.path;
    let tip = match prepared.settings.show {
        true => match pose_at(prepared, project, map, t, frame) {
            Some(pose) => pose.tip,
            None => place.on_frame(path.at(place.source))?,
        },
        false => place.on_frame(path.recorded(place.source))?,
    };
    Some(Pointer {
        tip,
        arrow: place.arrow,
        k,
    })
}

#[cfg(test)]
#[path = "pointer_tests.rs"]
mod tests;
