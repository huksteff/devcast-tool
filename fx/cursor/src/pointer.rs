//! Where the cursor callout hangs (docs/porting/clicks-callout.md §11): the drawn cursor when it
//! is shown, the recorded pointer when it is not; the cursor leaves it in the frame for the
//! callout (`look.rs`).

use dc_core::{OutputTime, PixelSize, Project, TimeMap};
use dc_render::look::Pointer;

use crate::pose::pose_at;
use crate::prepared::{Prepared, REFERENCE_WIDTH};

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
