//! Where the cursor is drawn at an output moment, which picture and how it tilts; with «loop» the
//! end of the video brings it back to the start (docs/porting/cursor.md §12, decision 000: the
//! time is not stretched, only the last 670 ms are replaced).

use dc_core::{
    CursorPicture, CursorShape, OutputTime, PixelSize, Project, SourceTime, TimeDelta, TimeMap,
};

use crate::prepared::{Place, Prepared};

/// The end of the video that loop replaces: the return, then a still cursor (v1).
const LOOP_TAIL: TimeDelta = TimeDelta::from_millis(670);
const LOOP_RETURN: TimeDelta = TimeDelta::from_millis(550);

/// The cursor in the frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Pose<'a> {
    /// Where the hotspot is, px of the frame.
    pub tip: [f64; 2],
    pub picture: &'a CursorPicture,
    /// Height of the arrow, px of the frame (`Place::arrow`).
    pub arrow: f64,
    /// Radians, clockwise.
    pub turn: f64,
}

/// The cursor at output moment `t`, with the loop applied; `None` when nothing is drawn.
pub(crate) fn pose_at<'a>(
    prepared: &'a Prepared,
    project: &Project,
    map: &'a TimeMap,
    t: OutputTime,
    frame: PixelSize,
) -> Option<Pose<'a>> {
    let tail = (OutputTime::ZERO + map.duration()) - LOOP_TAIL;
    if prepared.settings.looped && t >= tail && tail > OutputTime::ZERO {
        let pose = |at| pose(&prepared.place(project, map, at, frame)?);
        if let (Some(from), Some(home)) = (pose(tail), pose(OutputTime::ZERO)) {
            return Some(back_home(from, home, t.since(tail)));
        }
    }
    pose(&prepared.place(project, map, t, frame)?)
}

/// The cursor of the recording where `place` shows it.
pub(crate) fn pose<'a>(place: &Place<'a>) -> Option<Pose<'a>> {
    let path = &place.recording.path;
    Some(Pose {
        tip: place.on_frame(path.at(place.source))?,
        picture: picture_at(&place.recording.shapes, place.source)?,
        arrow: place.arrow,
        turn: path.turn(place.source),
    })
}

/// `elapsed` into the loop tail: from `from` back to `home` (easeOutQuint), then still there.
fn back_home<'a>(from: Pose<'a>, home: Pose<'a>, elapsed: TimeDelta) -> Pose<'a> {
    let q = (elapsed.as_secs_f64() / LOOP_RETURN.as_secs_f64()).min(1.0);
    let e = 1.0 - (1.0 - q).powi(5);
    let mix = |a: f64, b: f64| a + (b - a) * e;
    Pose {
        tip: [mix(from.tip[0], home.tip[0]), mix(from.tip[1], home.tip[1])],
        picture: if q < 1.0 { from.picture } else { home.picture },
        arrow: mix(from.arrow, home.arrow),
        turn: 0.0,
    }
}

/// The picture shown at `t`: of the last shape change not after it, or of the first one;
/// `None` while the cursor is hidden.
fn picture_at(shapes: &[CursorShape], t: SourceTime) -> Option<&CursorPicture> {
    let index = shapes.partition_point(|s| s.at <= t);
    shapes.get(index.saturating_sub(1))?.picture.as_ref()
}
