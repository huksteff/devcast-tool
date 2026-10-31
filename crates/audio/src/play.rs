//! Playing the mix: a [`Line`] between the thread that renders the [`Mixer`] ahead ([`feed`])
//! and the output device that takes it ([`Line::pull`], called by `output::Output` or by a test
//! sink). The line also is the playback clock: [`Line::heard`] is the number of frames the
//! device has played by a moment, so the picture follows the sound.
//!
//! One line per start of playback: frame 0 of the line is the output frame the feed started
//! from. The device gets silence where the queue is empty; that silence is not counted, so the
//! clock waits for the sound instead of running ahead of it.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use crate::error::AudioError;
use crate::mix::Mixer;
use crate::{CHANNELS, RATE};

/// Frames the feed keeps queued ahead of the device: 0.1 s, a few device periods.
pub const AHEAD: usize = RATE as usize / 10;

/// Frames rendered by one call of the mixer.
const CHUNK: usize = 1024;

/// Why a line plays no sound; the picture then goes on by the wall clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Off {
    /// The project has nothing to sound.
    Silent,
    /// The mix or the device failed: the message for the user.
    Failed(String),
}

/// Sound on its way from the feed to the device, and how much of it was played.
#[derive(Debug)]
pub struct Line {
    state: Mutex<State>,
    /// Signalled when the device took samples or the line closed.
    changed: Condvar,
    /// Factor of every sample, `f32` bits, 0..1.
    volume: AtomicU32,
}

#[derive(Debug, Default)]
struct State {
    /// Interleaved samples ([`CHANNELS`]) not yet given to the device.
    queue: VecDeque<f32>,
    /// Frames given to the device so far.
    played: u64,
    /// The first frame of the last device buffer and when the device plays it.
    stamp: Option<(u64, Instant)>,
    /// The largest [`Line::heard`] answered: the clock never goes back.
    told: u64,
    /// The feed has queued everything to the end of the project.
    fed: bool,
    closed: bool,
    off: Option<Off>,
}

impl Line {
    /// An empty line at `volume` (0..1).
    pub fn new(volume: f32) -> Self {
        Self {
            state: Mutex::default(),
            changed: Condvar::new(),
            volume: AtomicU32::new(volume.clamp(0.0, 1.0).to_bits()),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        // A panic while holding the lock leaves plain numbers and samples: still usable.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Volume of what the device gets from now on, 0..1.
    pub fn set_volume(&self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        self.volume.store(volume.to_bits(), Ordering::Relaxed);
    }

    /// The device asks for `out` (interleaved, [`CHANNELS`]), whose first frame it plays at
    /// `plays_at`: queued sound at the volume, silence where the queue ran dry.
    pub fn pull(&self, out: &mut [f32], plays_at: Instant) {
        let volume = f32::from_bits(self.volume.load(Ordering::Relaxed));
        let mut state = self.state();
        let taken = out.len().min(state.queue.len()) / CHANNELS * CHANNELS;
        for (sample, queued) in out.iter_mut().zip(state.queue.drain(..taken)) {
            *sample = queued * volume;
        }
        out[taken..].fill(0.0);
        state.stamp = Some((state.played, plays_at));
        state.played += (taken / CHANNELS) as u64;
        drop(state);
        self.changed.notify_all();
    }

    /// Frames of the line the device has played by `now`: from the last device buffer on at the
    /// rate of the sound, never more than it was given and never less than told before.
    pub fn heard(&self, now: Instant) -> u64 {
        let mut state = self.state();
        let Some((first, plays_at)) = state.stamp else {
            return state.told;
        };
        let since = if now >= plays_at {
            now.duration_since(plays_at).as_secs_f64()
        } else {
            -plays_at.duration_since(now).as_secs_f64()
        };
        let estimate = (first as f64 + since * f64::from(RATE)).max(0.0) as u64;
        let heard = estimate.min(state.played).max(state.told);
        state.told = heard;
        heard
    }

    /// Everything to the end of the project has been played by `now`.
    pub fn finished(&self, now: Instant) -> bool {
        let done = {
            let state = self.state();
            state.fed && state.queue.is_empty()
        };
        done && self.heard(now) >= self.state().played
    }

    /// Stops the line: the feed returns, the device gets silence.
    pub fn close(&self) {
        self.state().closed = true;
        self.changed.notify_all();
    }

    /// The line plays no sound, for `why`; closes it.
    pub fn turn_off(&self, why: Off) {
        self.state().off = Some(why);
        self.close();
    }

    pub fn is_closed(&self) -> bool {
        self.state().closed
    }

    /// Why the line plays no sound, once it gave up.
    pub fn off(&self) -> Option<Off> {
        self.state().off.clone()
    }

    /// Waits until the line is closed.
    pub fn wait_closed(&self) {
        let state = self.state();
        let _closed = self
            .changed
            .wait_while(state, |s| !s.closed)
            .unwrap_or_else(PoisonError::into_inner);
    }

    /// Waits until the queue has room; the frames to add, `None` once the line is closed.
    fn room(&self) -> Option<usize> {
        let state = self.state();
        let state = self
            .changed
            .wait_while(state, |s| !s.closed && s.queue.len() >= AHEAD * CHANNELS)
            .unwrap_or_else(PoisonError::into_inner);
        (!state.closed).then(|| AHEAD - state.queue.len() / CHANNELS)
    }

    fn push(&self, samples: &[f32]) {
        self.state().queue.extend(samples);
    }

    fn fed(&self) {
        self.state().fed = true;
    }
}

/// Renders the output of `mixer` from frame `from` into `line` until the end of the project or
/// until the line is closed.
pub fn feed(mixer: &mut Mixer, line: &Line, from: i64) -> Result<(), AudioError> {
    let end = mixer.frames();
    let mut at = from.max(0);
    let mut buffer = Vec::new();
    while at < end {
        let Some(room) = line.room() else {
            return Ok(());
        };
        let left = usize::try_from(end - at).unwrap_or(usize::MAX);
        let frames = room.min(CHUNK).min(left);
        buffer.resize(frames * CHANNELS, 0.0);
        mixer.render(at, &mut buffer)?;
        line.push(&buffer);
        at += frames as i64;
    }
    line.fed();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn heard_runs_at_the_sound_rate_up_to_what_was_played() {
        let line = Line::new(1.0);
        line.push(&vec![0.5; 4800 * CHANNELS]);
        let t0 = Instant::now();
        let plays_at = t0 + Duration::from_millis(20);
        line.pull(&mut vec![0.0; 960 * CHANNELS], plays_at);
        assert_eq!(line.heard(t0), 0, "latency: not heard yet");
        assert_eq!(line.heard(plays_at + Duration::from_millis(10)), 480);
        assert_eq!(
            line.heard(plays_at + Duration::from_secs(1)),
            960,
            "never more than given to the device"
        );
        assert_eq!(line.heard(plays_at), 960, "never back");
    }

    #[test]
    fn the_volume_is_clamped() {
        let line = Line::new(3.0);
        line.push(&[1.0, -1.0]);
        let mut out = [0.0; 2];
        line.pull(&mut out, Instant::now());
        assert_eq!(out, [1.0, -1.0]);
        line.set_volume(-1.0);
        line.push(&[1.0, 1.0]);
        line.pull(&mut out, Instant::now());
        assert_eq!(out, [0.0, 0.0]);
    }
}
