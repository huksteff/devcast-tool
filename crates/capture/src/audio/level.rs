//! Level of a microphone or of the system sound for its indicator: RMS and peak of each 50 ms
//! of sound, without recording anything. System sound in silence sends no levels (loopback
//! delivers nothing then): the indicator treats an old level as silence.

use std::sync::mpsc::Sender;

use super::device::{AudioSource, Device};
use crate::error::CaptureError;

/// Levels per second: every 50 ms of sound.
pub const LEVELS_PER_SECOND: u32 = 20;

/// Quieter than this is shown as silence.
pub const FLOOR_DB: f32 = -100.0;

/// Loudness of one 50 ms window, dBFS (0 = full scale, [`FLOOR_DB`] = silence).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioLevel {
    pub rms_db: f32,
    pub peak_db: f32,
}

/// Measures while it lives; dropping it closes the device.
pub struct LevelMeter {
    _stream: cpal::Stream,
}

impl LevelMeter {
    /// Opens `source` and sends its level to `levels` [`LEVELS_PER_SECOND`] times
    /// a second, or an error when the device fails (then levels stop). A glitch only costs a
    /// level and is not reported.
    pub fn start(
        source: &AudioSource,
        levels: Sender<Result<AudioLevel, CaptureError>>,
    ) -> Result<Self, CaptureError> {
        let device = Device::find(source)?;
        let mut window = Window::new(device.rate() / LEVELS_PER_SECOND, device.channels());
        let (errors, name) = (levels.clone(), device.name());
        let stream = device.start(
            move |_, samples| {
                window.add(samples, |level| {
                    // The receiver is gone only when nobody shows the indicator any more.
                    let _ = levels.send(Ok(level));
                });
            },
            move |error| {
                if error.kind() != cpal::ErrorKind::Xrun {
                    let _ = errors.send(Err(CaptureError::audio_device(&name, error)));
                }
            },
        )?;
        Ok(Self { _stream: stream })
    }
}

/// Sums of the current window.
struct Window {
    frames: u32,
    channels: usize,
    seen: u32,
    squares: f64,
    peak: f32,
}

impl Window {
    fn new(frames: u32, channels: usize) -> Self {
        Self {
            frames,
            channels,
            seen: 0,
            squares: 0.0,
            peak: 0.0,
        }
    }

    /// Adds interleaved `samples`; each full window becomes a level.
    fn add(&mut self, samples: &[f32], mut done: impl FnMut(AudioLevel)) {
        for frame in samples.chunks_exact(self.channels) {
            for sample in frame {
                self.squares += f64::from(*sample) * f64::from(*sample);
                self.peak = self.peak.max(sample.abs());
            }
            self.seen += 1;
            if self.seen == self.frames {
                let count = f64::from(self.frames) * self.channels as f64;
                done(AudioLevel {
                    rms_db: decibels((self.squares / count).sqrt() as f32),
                    peak_db: decibels(self.peak),
                });
                (self.seen, self.squares, self.peak) = (0, 0.0, 0.0);
            }
        }
    }
}

fn decibels(amplitude: f32) -> f32 {
    (20.0 * amplitude.log10()).max(FLOOR_DB)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_windows_become_levels() {
        let mut window = Window::new(4, 2);
        let mut levels = Vec::new();
        // A full-scale square wave on the left, silence on the right; then silence.
        window.add(&[1.0, 0.0, -1.0, 0.0, 1.0, 0.0], |l| levels.push(l));
        assert!(levels.is_empty());
        window.add(&[-1.0, 0.0, 0.0, 0.0, 0.0, 0.0], |l| levels.push(l));
        window.add(&[0.0; 4], |l| levels.push(l));
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0].peak_db, 0.0);
        assert!((levels[0].rms_db + 3.01).abs() < 0.01, "{levels:?}");
        assert_eq!(
            levels[1],
            AudioLevel {
                rms_db: FLOOR_DB,
                peak_db: FLOOR_DB,
            }
        );
    }
}
