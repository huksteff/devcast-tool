//! A project with one recording and its shortcuts, shared by the tests of this crate.

use dc_core::project::{Media, MediaKind};
use dc_core::{
    Clip, KeyPress, KeyTracks, OutputTime, PixelSize, Project, Range, Rect, SourceTime, TimeDelta,
};

use crate::{KeysSettings, KeysState};

pub const RECORDING: &str = "media/recording-1/video.mp4";

pub const HD: PixelSize = PixelSize {
    width: 1920,
    height: 1080,
};

pub fn s(secs: f64) -> SourceTime {
    SourceTime::from_secs_f64(secs)
}

pub fn o(secs: f64) -> OutputTime {
    OutputTime::from_secs_f64(secs)
}

/// Settings with the plate shown.
pub fn shown() -> KeysState {
    KeysState {
        settings: KeysSettings {
            show: true,
            ..KeysSettings::default()
        },
    }
}

pub fn clip(project: &mut Project, path: &str, secs: f64) {
    let id = project.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(secs);
    project.clips.push(Clip {
        id,
        media: Media {
            path: path.to_owned(),
            kind: MediaKind::Recording,
            duration,
            size: HD,
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration).unwrap(),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
        audio: Default::default(),
    });
}

/// A 10 s recording, then an imported video of 2 s without telemetry.
pub fn project() -> Project {
    let mut p = Project::default();
    clip(&mut p, RECORDING, 10.0);
    clip(&mut p, "media/импорт 1.mp4", 2.0);
    p
}

/// Ctrl+S at 1 s; Ctrl+Z at 3, 3.4 and 3.8 s; Alt+Tab at 4.2 s (replaces it); F5 at 7 s and
/// again at 9 s (too late to count); Esc at 9.9 s (near the end of the recording).
pub fn tracks() -> KeyTracks {
    let press = |secs: f64, name: &str| KeyPress {
        at: s(secs),
        name: name.to_owned(),
    };
    let presses = vec![
        press(1.0, "Ctrl+S"),
        press(3.0, "Ctrl+Z"),
        press(3.4, "Ctrl+Z"),
        press(3.8, "Ctrl+Z"),
        press(4.2, "Alt+Tab"),
        press(7.0, "F5"),
        press(9.0, "F5"),
        press(9.9, "Esc"),
    ];
    KeyTracks::from([(RECORDING.to_owned(), presses)])
}
