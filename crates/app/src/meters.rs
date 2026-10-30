//! Live sound levels of the indicators (owner's decision): the selected microphone while
//! something shows it, every microphone and the system sound while the sound popover is open.
//! Each level wakes the main window, which passes the repaint on to the windows it shows
//! (`app::show_windows`); nothing is measured while nothing is shown.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use dc_capture::audio::{AudioLevel, AudioSource, LevelMeter};
use eframe::egui;

use crate::error::AppError;

/// A level older than this is silence: loopback sends nothing while nothing plays.
const STALE: Duration = Duration::from_millis(300);

/// Quieter than this (dBFS) is an empty indicator, 0 dBFS a full one.
const RANGE_DB: f32 = 60.0;

/// The last level of a source and when it came.
type Last = Arc<Mutex<Option<(AudioLevel, Instant)>>>;

struct Meter {
    _meter: LevelMeter,
    last: Last,
}

/// The open meters.
#[derive(Default)]
pub struct Meters {
    open: HashMap<String, Meter>,
    /// Sources that failed to open, not retried until they stop being wanted.
    failed: Vec<String>,
}

fn key(source: &AudioSource) -> String {
    match source {
        AudioSource::System => String::new(),
        AudioSource::Microphone(name) => name.clone(),
    }
}

impl Meters {
    /// Keeps exactly `wanted` open; returns the errors of sources that would not open.
    pub fn sync(&mut self, ctx: &egui::Context, wanted: &[AudioSource]) -> Vec<AppError> {
        let keys: Vec<String> = wanted.iter().map(key).collect();
        self.open.retain(|k, _| keys.contains(k));
        self.failed.retain(|k| keys.contains(k));
        let mut errors = Vec::new();
        for (source, key) in wanted.iter().zip(keys) {
            if self.open.contains_key(&key) || self.failed.contains(&key) {
                continue;
            }
            match open(ctx, source) {
                Ok(meter) => {
                    self.open.insert(key, meter);
                }
                Err(error) => {
                    self.failed.push(key);
                    errors.push(error);
                }
            }
        }
        errors
    }

    /// Share of the indicator to fill, 0..1; 0 for a source that is not measured.
    pub fn level(&self, ctx: &egui::Context, source: &AudioSource) -> f32 {
        let Some(meter) = self.open.get(&key(source)) else {
            return 0.0;
        };
        let last = *meter.last.lock().unwrap_or_else(PoisonError::into_inner);
        match last {
            Some((level, at)) if at.elapsed() < STALE => {
                ctx.request_repaint_after(STALE);
                share(level.rms_db)
            }
            _ => 0.0,
        }
    }
}

/// dBFS as a share of the indicator.
pub fn share(db: f32) -> f32 {
    ((db + RANGE_DB) / RANGE_DB).clamp(0.0, 1.0)
}

fn open(ctx: &egui::Context, source: &AudioSource) -> Result<Meter, AppError> {
    let (sender, levels) = mpsc::channel();
    let meter = LevelMeter::start(source, sender)?;
    let last: Last = Arc::default();
    let (ctx, shared) = (ctx.clone(), Arc::clone(&last));
    std::thread::Builder::new()
        .name("devcast-level".to_owned())
        .spawn(move || {
            // Ends when the meter is dropped: its stream holds the only senders.
            for level in levels {
                match level {
                    Ok(level) => {
                        *shared.lock().unwrap_or_else(PoisonError::into_inner) =
                            Some((level, Instant::now()));
                        ctx.request_repaint_of(egui::ViewportId::ROOT);
                    }
                    Err(error) => tracing::warn!("индикатор уровня остановился: {error}"),
                }
            }
        })
        .map_err(|source| AppError::Thread {
            name: "индикатор уровня",
            source,
        })?;
    Ok(Meter {
        _meter: meter,
        last,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn decibels_fill_the_indicator() {
        assert_eq!(super::share(0.0), 1.0);
        assert_eq!(super::share(-30.0), 0.5);
        assert_eq!(super::share(-100.0), 0.0);
    }
}
