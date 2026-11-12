//! A project with one recording and its cursor telemetry, shared by the tests of this crate.

use dc_core::project::{Media, MediaKind};
use dc_core::{
    Clip, CursorPicture, CursorPoint, CursorShape, CursorTrack, CursorTracks, OutputTime,
    PixelSize, Point, Project, Range, Rect, SourceTime, TimeDelta,
};

use crate::{CursorSettings, CursorState, Smoothing};

/// The lively look of v1 (big, on a spring, squeezed by clicks, tilted, blurred): what the
/// tests of motion are written for. The defaults are calm and have none of it.
pub fn lively() -> CursorSettings {
    CursorSettings {
        size: 2.5,
        smoothing: Smoothing::Focused,
        blur: 0.6,
        bounce: 2.0,
        sway: 0.4,
        ..CursorSettings::default()
    }
}

/// A state of these settings without changed clicks.
pub fn state(settings: CursorSettings) -> CursorState {
    CursorState {
        settings,
        clicks: Vec::new(),
    }
}

pub const RECORDING: &str = "media/recording-1/video.mp4";
pub const ARROW: &str = "media/recording-1/cursor-0000.png";
pub const HAND: &str = "media/recording-1/cursor-0001.png";

pub fn s(secs: f64) -> SourceTime {
    SourceTime::from_secs_f64(secs)
}

pub fn o(secs: f64) -> OutputTime {
    OutputTime::from_secs_f64(secs)
}

pub fn clip(project: &mut Project, path: &str, secs: f64, size: PixelSize) {
    let id = project.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(secs);
    project.clips.push(Clip {
        id,
        media: Media {
            path: path.to_owned(),
            kind: MediaKind::Recording,
            duration,
            size,
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration).unwrap(),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
        audio: Default::default(),
    });
}

pub const HD: PixelSize = PixelSize {
    width: 1920,
    height: 1080,
};

/// A 4 s 1080p recording, then a 4:3 imported video of 2 s without telemetry.
pub fn project() -> Project {
    let mut p = Project::default();
    clip(&mut p, RECORDING, 4.0, HD);
    let size = PixelSize {
        width: 1440,
        height: 1080,
    };
    clip(&mut p, "media/импорт 1.mp4", 2.0, size);
    p
}

fn picture(path: &str, hotspot: (f64, f64)) -> CursorPicture {
    CursorPicture {
        path: path.to_owned(),
        size: PixelSize {
            width: 32,
            height: 32,
        },
        hotspot: Point {
            x: hotspot.0,
            y: hotspot.1,
        },
    }
}

pub fn point(secs: f64, x: f64, y: f64) -> CursorPoint {
    CursorPoint {
        at: s(secs),
        pos: Point { x, y },
    }
}

/// The pointer rests at (0.2, 0.3), sweeps to (0.8, 0.7) over 0.5–1.0 s at 125 Hz, clicks there
/// at 1.2 s, jumps to (0.5, 0.5) at 2.5 s and leaves the picture at 3.6 s. An arrow until 2.5 s,
/// then a hand (hotspot 6, 0), hidden from 3.3 to 3.5 s.
pub fn track() -> CursorTrack {
    let mut moves = vec![point(0.0, 0.2, 0.3)];
    for i in 0..=62 {
        let q = f64::from(i) / 62.0;
        moves.push(point(0.5 + 0.5 * q, 0.2 + 0.6 * q, 0.3 + 0.4 * q));
    }
    let click = point(1.2, 0.8, 0.7);
    moves.extend([click, point(2.5, 0.5, 0.5), point(3.6, 1.2, 0.5)]);
    let shape = |secs, picture| CursorShape {
        at: s(secs),
        picture,
    };
    CursorTrack {
        moves,
        shapes: vec![
            shape(0.0, Some(picture(ARROW, (0.0, 0.0)))),
            shape(2.5, Some(picture(HAND, (6.0, 0.0)))),
            shape(3.3, None),
            shape(3.5, Some(picture(HAND, (6.0, 0.0)))),
        ],
        clicks: vec![click],
    }
}

pub fn tracks() -> CursorTracks {
    CursorTracks::from([(RECORDING.to_owned(), track())])
}
