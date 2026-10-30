//! The project's sound on the output timeline: every sound track of every segment of the time
//! map at its clip's level (and EBU R128 normalization), with crossfades at the joins and speed
//! through `atempo`, and the extra sound files at their places, summed and limited.
//!
//! One path for preview and export: both ask [`Mixer::render`] for output frames; preview
//! jumps around, export goes through in order. Each frame depends only on the project, not on
//! what was asked before (except the inner state of `atempo` after a jump, see `part`).

use std::collections::HashMap;

use dc_core::{Project, TimeMap};
use dc_project::ProjectDir;

use crate::error::AudioError;
use crate::file::file_track;
use crate::levels::{ChannelLevel, ExtraClip, Levels};
use crate::limiter::Limited;
use crate::loudness::LoudnessCache;
use crate::part::{Part, PartStream};
use crate::track::{SourceTrack, tracks};
use crate::{CHANNELS, frame_at};

pub struct Mixer {
    sum: Sum,
    limited: Limited,
    frames: i64,
}

/// The parts and their open streams; a stream is kept while its part keeps sounding.
struct Sum {
    parts: Vec<Part>,
    streams: Vec<Option<PartStream>>,
    scratch: Vec<f32>,
}

impl Mixer {
    /// The mix of `project`, whose folder is `project_dir`, at `levels`, with their extra sound;
    /// normalized tracks are measured into `loudness` unless they are there already.
    pub fn new(
        project_dir: &ProjectDir,
        project: &Project,
        levels: &Levels,
        loudness: &mut LoudnessCache,
    ) -> Result<Self, AudioError> {
        let map = TimeMap::build(project);
        let mut parts = clip_parts(project_dir, project, &map, levels, loudness)?;
        for extra in &levels.extra {
            parts.extend(extra_part(extra, loudness)?);
        }
        Ok(Self {
            sum: Sum {
                streams: parts.iter().map(|_| None).collect(),
                parts,
                scratch: Vec::new(),
            },
            limited: Limited::default(),
            frames: frame_at(map.duration().micros()),
        })
    }

    /// Length of the output, frames.
    pub fn frames(&self) -> i64 {
        self.frames
    }

    /// Fills `out` (interleaved, [`CHANNELS`]) with the output from frame `start`; silence
    /// outside `0..frames()`.
    pub fn render(&mut self, start: i64, out: &mut [f32]) -> Result<(), AudioError> {
        let sum = &mut self.sum;
        self.limited
            .render(start, out, |from, buffer| sum.fill(from, buffer))
    }
}

/// Every sound track of every segment of `map`, at its clip's level.
fn clip_parts(
    project_dir: &ProjectDir,
    project: &Project,
    map: &TimeMap,
    levels: &Levels,
    loudness: &mut LoudnessCache,
) -> Result<Vec<Part>, AudioError> {
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
            let level = levels.channel(segment.clip, &track.name);
            parts.push(Part {
                gain: gain(level, track, loudness)?,
                denoise: level.denoised(),
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
    Ok(parts)
}

/// The part of an extra sound file; none when it is silent or empty. Its edges fade like a
/// join ([`crate::CROSSFADE`]), so a file cut short or started inside does not click.
fn extra_part(extra: &ExtraClip, loudness: &mut LoudnessCache) -> Result<Option<Part>, AudioError> {
    if extra.end <= extra.start || extra.level.factor() == 0.0 {
        return Ok(None);
    }
    let track = file_track(&extra.path)?;
    Ok(Some(Part {
        gain: gain(extra.level, &track, loudness)?,
        denoise: extra.level.denoised(),
        start: frame_at(extra.start.micros()),
        end: frame_at(extra.end.micros()),
        source: frame_at(extra.offset.micros()),
        track,
        speed: 1.0,
        join_before: true,
        join_after: true,
    }))
}

/// Factor of `level` on `track`, with its normalization.
fn gain(
    level: ChannelLevel,
    track: &SourceTrack,
    loudness: &mut LoudnessCache,
) -> Result<f32, AudioError> {
    let normalization = if level.normalized() && level.factor() > 0.0 {
        loudness.normalization(track)?
    } else {
        1.0
    };
    Ok(level.factor() * normalization)
}

impl Sum {
    /// The sum of the parts from output frame `start`, before the limiter.
    fn fill(&mut self, start: i64, out: &mut [f32]) -> Result<(), AudioError> {
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
