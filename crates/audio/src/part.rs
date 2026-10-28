//! One sound track of one segment of the time map, as it sounds on the output: its frames in
//! the source, its speed and the crossfades with the neighbor segments.
//!
//! A join between segments is a crossfade of [`CROSSFADE`] frames centered on it (decision
//! 000, «Звук»): the segment before plays on past its end (the cut-out sound) fading out, the
//! next one starts before its start fading in. Timing stays exact, and the two halves add up
//! to one, so a join inside continuous sound (a speed change, a split clip) is not heard.

use crate::error::AudioError;
use crate::reader::TrackReader;
use crate::tempo::Tempo;
use crate::track::SourceTrack;
use crate::{CHANNELS, RATE};

/// Length of the crossfade at a join, frames (10 ms).
pub const CROSSFADE: i64 = RATE as i64 / 100;

const HALF: i64 = CROSSFADE / 2;

/// `atempo` starts this many output frames before the asked one and they are dropped, so that
/// its window is full where the output begins.
const TEMPO_WARMUP: i64 = RATE as i64 / 10;

/// Source frames fed to `atempo` at a time.
const TEMPO_BLOCK: usize = 4096;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Part {
    pub track: SourceTrack,
    /// Output frames of the segment, `start..end`.
    pub start: i64,
    pub end: i64,
    /// Source frame played at `start`.
    pub source: i64,
    pub speed: f64,
    /// A neighbor segment joins at the start / at the end.
    pub join_before: bool,
    pub join_after: bool,
}

impl Part {
    /// Output frames where the part sounds: the segment and the crossfades at its joins.
    pub fn span(&self) -> (i64, i64) {
        let before = if self.join_before { HALF } else { 0 };
        let after = if self.join_after { HALF } else { 0 };
        (self.start - before, self.end + after)
    }

    /// Crossfade gain at output frame `k`.
    pub fn envelope(&self, k: i64) -> f32 {
        let rise = |from: i64| {
            let x = ((k - from) as f64 / CROSSFADE as f64).clamp(0.0, 1.0);
            (0.5 - 0.5 * (std::f64::consts::PI * x).cos()) as f32
        };
        let fade_in = if self.join_before {
            rise(self.start - HALF)
        } else {
            1.0
        };
        let fade_out = if self.join_after {
            1.0 - rise(self.end - HALF)
        } else {
            1.0
        };
        fade_in * fade_out
    }
}

/// Reads a [`Part`] in output frames: speed 1 straight from the track, other speeds through
/// `atempo` (output frame `start + j` plays source `source + j · speed`).
pub(crate) struct PartStream {
    reader: TrackReader,
    tempo: Option<TempoRun>,
}

struct TempoRun {
    tempo: Tempo,
    /// Output frame of `ready[0]`.
    next: i64,
    /// Next source frame to feed.
    source_next: i64,
    ready: Vec<f32>,
    feed: Vec<f32>,
}

impl PartStream {
    pub fn open(part: &Part) -> Result<Self, AudioError> {
        Ok(Self {
            reader: TrackReader::open(&part.track)?,
            tempo: None,
        })
    }

    /// Fills `out` with the part's frames from output frame `start` (no envelope).
    pub fn read(&mut self, part: &Part, start: i64, out: &mut [f32]) -> Result<(), AudioError> {
        if part.speed == 1.0 {
            return self.reader.fill(part.source + (start - part.start), out);
        }
        let run = match &mut self.tempo {
            Some(run) if run.next == start => run,
            slot => slot.insert(TempoRun::start(part, start, &mut self.reader)?),
        };
        run.read(&mut self.reader, out)
    }
}

impl TempoRun {
    /// A new `atempo` run whose next output frame is `start`.
    fn start(part: &Part, start: i64, reader: &mut TrackReader) -> Result<Self, AudioError> {
        let first = start - TEMPO_WARMUP;
        let offset = ((first - part.start) as f64 * part.speed).round() as i64;
        let mut run = Self {
            tempo: Tempo::new(part.speed)?,
            next: first,
            source_next: part.source + offset,
            ready: Vec::new(),
            feed: vec![0.0; TEMPO_BLOCK * CHANNELS],
        };
        run.read(reader, &mut vec![0.0; TEMPO_WARMUP as usize * CHANNELS])?;
        Ok(run)
    }

    fn read(&mut self, reader: &mut TrackReader, out: &mut [f32]) -> Result<(), AudioError> {
        while self.ready.len() < out.len() {
            reader.fill(self.source_next, &mut self.feed)?;
            self.tempo.push(&self.feed, &mut self.ready)?;
            self.source_next += TEMPO_BLOCK as i64;
        }
        out.copy_from_slice(&self.ready[..out.len()]);
        self.ready.drain(..out.len());
        self.next += (out.len() / CHANNELS) as i64;
        Ok(())
    }
}
