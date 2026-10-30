//! Builders shared by the tests of this crate.

use crate::geom::{PixelSize, Rect};
use crate::project::{Clip, ClipId, Media, MediaKind, Project};
use crate::time::{Range, Source, SourceTime, TimeDelta};

pub fn t(secs: f64) -> SourceTime {
    SourceTime::from_secs_f64(secs)
}

pub fn r(a: f64, b: f64) -> Range<Source> {
    Range::new(t(a), t(b)).unwrap()
}

/// Adds a clip of a whole new recording, `duration` seconds long.
pub fn clip(project: &mut Project, duration: f64) -> ClipId {
    let id = project.clip_ids.allocate();
    project.clips.push(Clip {
        id,
        media: Media {
            path: format!("media/{id}.mp4"),
            kind: MediaKind::Recording,
            duration: TimeDelta::from_secs_f64(duration),
            size: PixelSize {
                width: 1920,
                height: 1080,
            },
        },
        trim: r(0.0, duration),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
        audio: Default::default(),
    });
    id
}
