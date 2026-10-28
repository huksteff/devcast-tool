//! Silence: stretches where the sound is quieter than a threshold, measured as RMS of 50 ms
//! blocks over all channels. Not FFmpeg `silencedetect` (every sample below the threshold):
//! a raised microphone hisses with single samples above −30 dBFS and would have no pauses at
//! all (owner's speech fixture: noise −35 dB RMS). Defaults are v1's (decision 000): −30 dBFS,
//! at least 0.5 s, on the microphone track.

use dc_core::time::Source;
use dc_core::{Range, SourceTime, TimeDelta};

use crate::error::AudioError;
use crate::reader::TrackReader;
use crate::track::SourceTrack;
use crate::{CHANNELS, RATE, frame_at};

pub const DEFAULT_DB: f64 = -30.0;
pub const DEFAULT_MIN: TimeDelta = TimeDelta::from_millis(500);

/// Frames of one loudness block: 50 ms.
pub const BLOCK: usize = RATE as usize / 20;

/// Frames read at a time.
const CHUNK: usize = BLOCK * 5;

/// Finds quiet runs in frames pushed in order, block by block from the first frame.
#[derive(Debug, Clone)]
pub struct Detector {
    /// Threshold as a mean square.
    threshold: f64,
    min_frames: i64,
    first: i64,
    /// Sum of squares and samples of the block being filled.
    block: (f64, usize),
    /// First frame of the block being filled.
    next: i64,
    /// First frame of the quiet run in progress.
    run: Option<i64>,
    found: Vec<(i64, i64)>,
}

impl Detector {
    /// Quiet = RMS of a block below `db` dBFS; runs shorter than `min_frames` are dropped,
    /// except the ones touching the first or the last frame (they may go on past them).
    pub fn new(db: f64, min_frames: i64, first: i64) -> Self {
        Self {
            threshold: 10f64.powf(db / 10.0),
            min_frames,
            first,
            block: (0.0, 0),
            next: first,
            run: None,
            found: Vec::new(),
        }
    }

    /// Takes interleaved [`CHANNELS`] frames that follow the previous ones.
    pub fn push(&mut self, samples: &[f32]) {
        for sample in samples {
            self.block.0 += f64::from(*sample) * f64::from(*sample);
            self.block.1 += 1;
            if self.block.1 == BLOCK * CHANNELS {
                self.close_block();
            }
        }
    }

    fn close_block(&mut self) {
        let (sum, count) = std::mem::take(&mut self.block);
        if count == 0 {
            return;
        }
        let quiet = sum / (count as f64) < self.threshold;
        match (quiet, self.run) {
            (true, None) => self.run = Some(self.next),
            (false, Some(start)) => {
                self.found.push((start, self.next));
                self.run = None;
            }
            _ => {}
        }
        self.next += (count / CHANNELS) as i64;
    }

    /// Quiet runs as `[first, end)` frames, in order.
    pub fn finish(mut self) -> Vec<(i64, i64)> {
        self.close_block();
        if let Some(start) = self.run.take() {
            self.found.push((start, self.next));
        }
        let (first, last, min) = (self.first, self.next, self.min_frames);
        self.found
            .into_iter()
            .filter(|&(start, end)| end - start >= min || start == first || end == last)
            .collect()
    }
}

/// Quiet runs of `track` inside `range` of its source, at least `min` long unless they touch
/// an edge of `range` (a cut or clip boundary may join them with the neighbour).
pub fn find(
    track: &SourceTrack,
    range: Range<Source>,
    db: f64,
    min: TimeDelta,
) -> Result<Vec<Range<Source>>, AudioError> {
    let (first, last) = (
        frame_at(range.start().micros()),
        frame_at(range.end().micros()),
    );
    let mut reader = TrackReader::open(track)?;
    let mut detector = Detector::new(db, frame_at(min.micros()), first);
    let mut buffer = vec![0.0; CHUNK * CHANNELS];
    let mut at = first;
    while at < last {
        let frames = ((last - at) as usize).min(CHUNK);
        let part = &mut buffer[..frames * CHANNELS];
        reader.fill(at, part)?;
        detector.push(part);
        at += frames as i64;
    }
    let micros = |frame: i64| SourceTime::from_micros(frame * 1_000_000 / i64::from(RATE));
    Ok(detector
        .finish()
        .into_iter()
        .filter_map(|(start, end)| {
            let end = if end == last {
                range.end()
            } else {
                micros(end)
            };
            let start = if start == first {
                range.start()
            } else {
                micros(start)
            };
            Range::new(start, end).ok()
        })
        .collect())
}

/// Frames of the loudness window when looking for the quietest moment: 10 ms.
const QUIET_WINDOW: usize = RATE as usize / 100;

/// The quietest moment of `track` within `reach` of `around` (the middle of the 10 ms window
/// with the least RMS, in 1 ms steps): where a cut between words does not split a sound.
pub fn quietest(
    track: &SourceTrack,
    around: SourceTime,
    reach: TimeDelta,
) -> Result<SourceTime, AudioError> {
    let center = frame_at(around.micros());
    let reach = frame_at(reach.micros());
    let first = center - reach - QUIET_WINDOW as i64 / 2;
    let frames = (2 * reach) as usize + QUIET_WINDOW;
    let mut samples = vec![0.0; frames * CHANNELS];
    TrackReader::open(track)?.fill(first, &mut samples)?;
    let step = RATE as usize / 1000;
    let energy = |at: usize| -> f64 {
        samples[at * CHANNELS..(at + QUIET_WINDOW) * CHANNELS]
            .iter()
            .map(|s| f64::from(*s).powi(2))
            .sum()
    };
    // Ties go to the moment closest to `around`.
    let best = (0..=2 * reach as usize)
        .step_by(step)
        .min_by(|a, b| {
            energy(*a).total_cmp(&energy(*b)).then_with(|| {
                let distance = |x: usize| (x as i64 - reach).abs();
                distance(*a).cmp(&distance(*b))
            })
        })
        .unwrap_or(reach as usize);
    let frame = first + (best + QUIET_WINDOW / 2) as i64;
    Ok(SourceTime::from_micros(frame * 1_000_000 / i64::from(RATE)))
}
