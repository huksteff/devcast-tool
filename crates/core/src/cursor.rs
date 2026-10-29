//! Cursor telemetry of the recordings, as the cursor effect reads it (decision 015, «Телеметрия
//! курсора»). The files of a recording (`cursor.bin`, `cursor-NNNN.png`) belong to `dc-capture`;
//! the top layer reads them into these types and hands them to `dc_ops::Evaluator`, because
//! effects see only the core.

use std::collections::BTreeMap;

use crate::geom::{PixelSize, Point};
use crate::time::SourceTime;

/// Cursor telemetry of every recording on the track, by `Media::path` of its video.
pub type CursorTracks = BTreeMap<String, CursorTrack>;

/// What the mouse did during one recording, each list in time order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CursorTrack {
    /// Where the pointer was at each mouse event; it stays there until the next one.
    pub moves: Vec<CursorPoint>,
    /// What the cursor looked like from each moment on.
    pub shapes: Vec<CursorShape>,
    /// Presses of any mouse button.
    pub clicks: Vec<CursorPoint>,
}

/// The pointer at a moment of the recording.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorPoint {
    pub at: SourceTime,
    /// Fractions of the recorded picture, y down; outside 0..1 off the picture.
    pub pos: Point,
}

/// From `at` on the cursor shows `picture`, or is hidden (`None`).
#[derive(Debug, Clone, PartialEq)]
pub struct CursorShape {
    pub at: SourceTime,
    pub picture: Option<CursorPicture>,
}

/// A picture of the system cursor captured during the recording.
#[derive(Debug, Clone, PartialEq)]
pub struct CursorPicture {
    /// Path of the PNG in the project (`media/recording-1/cursor-0003.png`).
    pub path: String,
    pub size: PixelSize,
    /// The point that clicks, px of the picture from its top-left corner.
    pub hotspot: Point,
}
