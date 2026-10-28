//! Input telemetry of a finished recording on its source time: what the editor and `devcast`
//! read. Tick 0 of the recording is the start of its video track in the manifest; the event
//! files are already on the recording clock (pauses cut out), so source time is the event's
//! host time minus that start. Positions stay in pixels of the recorded picture.

use std::path::Path;

use dc_core::{SourceTime, TimeDelta};

use super::cursor::{Button, CursorEvent, read_cursor};
use super::keys::{KeyEvent, read_keys};
use crate::error::CaptureError;
use crate::event::HostTime;
use crate::manifest::{MANIFEST, Manifest, VIDEO_TRACK};

/// Two presses of one button this close in time and space are a double click (the Windows
/// defaults: `GetDoubleClickTime` 500 ms, `SM_CXDOUBLECLK` × `SM_CYDOUBLECLK` 4 × 4 px).
pub const DOUBLE_CLICK_TIME: TimeDelta = TimeDelta::from_millis(500);
pub const DOUBLE_CLICK_DISTANCE: i32 = 2;

/// An event at a moment of the recording's source time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceEvent<E> {
    pub at: SourceTime,
    pub event: E,
}

/// Mouse events, cursor shapes and shortcuts of one recording, in time order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Telemetry {
    pub cursor: Vec<SourceEvent<CursorEvent>>,
    pub keys: Vec<SourceEvent<KeyEvent>>,
}

/// A press of a mouse button at pixel (`x`, `y`) of the recorded picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Click {
    pub at: SourceTime,
    pub button: Button,
    pub x: i32,
    pub y: i32,
    /// The second press of a double click.
    pub double: bool,
}

impl Telemetry {
    /// Reads `cursor.bin` and `keys.bin` of the finished recording in folder `dir`.
    pub fn read(dir: &Path) -> Result<Self, CaptureError> {
        let manifest = Manifest::read(dir)?;
        let video = manifest
            .tracks
            .iter()
            .find(|t| t.name == VIDEO_TRACK)
            .ok_or_else(|| CaptureError::BadManifest {
                path: dir.join(MANIFEST),
                message: "нет видео-дорожки".to_owned(),
            })?;
        let start = video.start;
        let at = |time: HostTime| SourceTime::from_micros(time.micros() - start.micros());
        Ok(Self {
            cursor: read_cursor(dir)?
                .into_iter()
                .map(|e| SourceEvent {
                    at: at(e.at),
                    event: e.event,
                })
                .collect(),
            keys: read_keys(dir)?
                .into_iter()
                .map(|e| SourceEvent {
                    at: at(e.at),
                    event: e.event,
                })
                .collect(),
        })
    }

    /// Every button press in time order. A press is `double` if the previous press of the same
    /// button, itself not the second of a double click, was within [`DOUBLE_CLICK_TIME`] and
    /// [`DOUBLE_CLICK_DISTANCE`] pixels on both axes.
    pub fn clicks(&self) -> Vec<Click> {
        let mut clicks: Vec<Click> = Vec::new();
        for event in &self.cursor {
            let CursorEvent::Down { button, x, y } = event.event else {
                continue;
            };
            let double = clicks.last().is_some_and(|last| {
                !last.double
                    && last.button == button
                    && event.at.since(last.at) <= DOUBLE_CLICK_TIME
                    && (x - last.x).abs() <= DOUBLE_CLICK_DISTANCE
                    && (y - last.y).abs() <= DOUBLE_CLICK_DISTANCE
            });
            clicks.push(Click {
                at: event.at,
                button,
                x,
                y,
                double,
            });
        }
        clicks
    }
}

#[cfg(test)]
#[path = "telemetry_tests.rs"]
mod tests;
