//! The project's sound on the output timeline: every sound track of every segment of the time
//! map at its clip's level, with crossfades at the joins and speed through `atempo`, summed and
//! limited.
//!
//! One path for preview and export: both ask [`Mixer::render`] for output frames; preview
//! jumps around, export goes through in order. Each frame depends only on the project, not on
//! what was asked before (except the inner state of `atempo` after a jump, see `part`).

use std::collections::HashMap;
use std::path::Path;

use dc_core::{Project, TimeMap};

use crate::error::AudioError;
use crate::levels::Levels;
use crate::limiter::{AFTER, BEFORE, Limiter};
use crate::part::{Part, PartStream};
use crate::track::{SourceTrack, tracks};
use crate::{CHANNELS, frame_at};

pub struct Mixer {
    parts: Vec<Part>,
    /// Open stream of each part that sounded in the last render.
    streams: Vec<Option<PartStream>>,
    frames: i64,
    scratch: Vec<f32>,
    /// The sum before the limiter, from frame `premix_start`: the limiter's context is kept
    /// for the next render that goes on in order.
    premix: Vec<f32>,
    premix_start: i64,
    limiter: Limiter,
}

impl Mixer {
    /// The mix of `project`, whose folder is `project_dir`, at `levels`.
    pub fn new(project_dir: &Path, project: &Project, levels: &Levels) -> Result<Self, AudioError> {
        let map = TimeMap::build(project);
        let mut media_tracks: HashMap<&str, Vec<SourceTrack>> = HashMap::new();
        for clip in &project.clips {
            if !media_tracks.contains_key(clip.media.path.as_str()) {
                let found = tracks(project_dir, &clip.media)?;
                media_tracks.insert(&clip.media.path, found);
            }
        }
        let segments = map.segments();
        let mut parts = Vec::new();
        for (index, segment) in segments.iter().enumerate() {
            for track in &media_tracks[segment.media.as_str()] {
                parts.push(Part {
                    gain: levels.factor(segment.clip, &track.name),
                    track: track.clone(),
                    start: frame_at(segment.output_start.micros()),
                    end: frame_at(segment.output_end().micros()),
                    source: frame_at(segment.source.start().micros()),
                    speed: segment.speed,
                    join_before: index > 0,
                    join_after: index + 1 < segments.len(),
                });
            }
        }
        Ok(Self {
            streams: parts.iter().map(|_| None).collect(),
            parts,
            frames: frame_at(map.duration().micros()),
            scratch: Vec::new(),
            premix: Vec::new(),
            premix_start: 0,
            limiter: Limiter::default(),
        })
    }

    /// Length of the output, frames.
    pub fn frames(&self) -> i64 {
        self.frames
    }

    /// Fills `out` (interleaved, [`CHANNELS`]) with the output from frame `start`; silence
    /// outside `0..frames()`.
    pub fn render(&mut self, start: i64, out: &mut [f32]) -> Result<(), AudioError> {
        let from = start - BEFORE as i64;
        let to = start + (out.len() / CHANNELS + AFTER) as i64;
        let premix_end = self.premix_start + (self.premix.len() / CHANNELS) as i64;
        if (self.premix_start..=premix_end).contains(&from) {
            let passed = (from - self.premix_start) as usize;
            self.premix.drain(..passed * CHANNELS);
            self.limiter.advance(passed);
        } else {
            self.premix.clear();
            self.limiter.reset();
        }
        self.premix_start = from;
        let have = from + (self.premix.len() / CHANNELS) as i64;
        if have < to {
            let mut premix = std::mem::take(&mut self.premix);
            let old = premix.len();
            premix.resize((to - from) as usize * CHANNELS, 0.0);
            self.sum(have, &mut premix[old..])?;
            self.premix = premix;
        }
        let window = (to - from) as usize * CHANNELS;
        self.limiter.apply(&self.premix[..window], out);
        Ok(())
    }

    /// The sum of the parts from output frame `start`, before the limiter.
    fn sum(&mut self, start: i64, out: &mut [f32]) -> Result<(), AudioError> {
        out.fill(0.0);
        let end = start + (out.len() / CHANNELS) as i64;
        for (part, stream) in self.parts.iter().zip(&mut self.streams) {
            let (from, to) = part.span();
            let (lo, hi) = (from.max(start), to.min(end));
            if lo >= hi || part.gain == 0.0 {
                *stream = None;
                continue;
            }
            let stream = match stream {
                Some(stream) => stream,
                None => stream.insert(PartStream::open(part)?),
            };
            let len = (hi - lo) as usize * CHANNELS;
            self.scratch.resize(len, 0.0);
            stream.read(part, lo, &mut self.scratch)?;
            let at = (lo - start) as usize * CHANNELS;
            let frames = self.scratch.as_chunks::<CHANNELS>().0.iter().zip(lo..);
            for ((frame, k), target) in frames.zip(out[at..].as_chunks_mut::<CHANNELS>().0) {
                let gain = part.envelope(k) * part.gain;
                for (o, s) in target.iter_mut().zip(frame) {
                    *o += s * gain;
                }
            }
        }
        Ok(())
    }
}
