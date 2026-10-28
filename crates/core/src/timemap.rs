//! The time map: where every moment of the output comes from. Built from the clips on the main
//! track, their cuts and speed ranges; converts between [`SourceTime`] (per clip) and
//! [`OutputTime`]. Effects stay in source time, so cutting never moves them.

use crate::project::{Clip, ClipId, Project};
use crate::time::{OutputTime, Range, Source, SourceTime, TimeDelta};

/// A part of one clip's source played at a constant speed, placed on the output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub clip: ClipId,
    pub source: Range<Source>,
    /// Playback factor; 2.0 plays twice as fast.
    pub speed: f64,
    pub output_start: OutputTime,
    pub output_len: TimeDelta,
}

impl Segment {
    pub fn output_end(&self) -> OutputTime {
        self.output_start + self.output_len
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimeMap {
    segments: Vec<Segment>,
    duration: TimeDelta,
}

/// `source_len / speed`, rounded to the nearest microsecond.
fn scaled(source_len: TimeDelta, speed: f64) -> TimeDelta {
    TimeDelta::from_micros((source_len.micros() as f64 / speed).round() as i64)
}

impl TimeMap {
    pub fn build(project: &Project) -> Self {
        let mut segments = Vec::new();
        let mut cursor = OutputTime::ZERO;
        for clip in &project.clips {
            for (source, speed) in pieces(clip) {
                let output_len = scaled(source.duration(), speed);
                if output_len <= TimeDelta::ZERO {
                    continue;
                }
                segments.push(Segment {
                    clip: clip.id,
                    source,
                    speed,
                    output_start: cursor,
                    output_len,
                });
                cursor = cursor + output_len;
            }
        }
        Self {
            segments,
            duration: cursor.since(OutputTime::ZERO),
        }
    }

    pub fn duration(&self) -> TimeDelta {
        self.duration
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Output time of source moment `t` of `clip`; `None` if it is cut or outside the trim.
    pub fn to_output(&self, clip: ClipId, t: SourceTime) -> Option<OutputTime> {
        let segment = self
            .segments
            .iter()
            .find(|s| s.clip == clip && s.source.contains(t))?;
        let offset = scaled(t.since(segment.source.start()), segment.speed);
        Some((segment.output_start + offset).min(segment.output_end() - TimeDelta::from_micros(1)))
    }

    /// Which clip and source moment is shown at output time `t`; `None` outside `0..duration`.
    pub fn to_source(&self, t: OutputTime) -> Option<(ClipId, SourceTime)> {
        let index = self.segments.partition_point(|s| s.output_end() <= t);
        let segment = self.segments.get(index).filter(|s| s.output_start <= t)?;
        let offset = t.since(segment.output_start).micros() as f64 * segment.speed;
        let source = segment.source.start() + TimeDelta::from_micros(offset.round() as i64);
        let last = segment.source.end() - TimeDelta::from_micros(1);
        Some((segment.clip, source.min(last)))
    }
}

/// The kept parts of `clip` (trim minus cuts), each split by speed ranges, in source order.
fn pieces(clip: &Clip) -> Vec<(Range<Source>, f64)> {
    let mut kept = Vec::new();
    let mut start = clip.trim.start();
    for cut in &clip.cuts {
        if cut.start() > start {
            push_range(&mut kept, start, cut.start().min(clip.trim.end()));
        }
        start = start.max(cut.end());
    }
    push_range(&mut kept, start, clip.trim.end());

    let mut out = Vec::new();
    for part in kept {
        let mut at = part.start();
        for speed in clip
            .speeds
            .iter()
            .filter(|s| s.range.intersect(&part).is_some())
        {
            let overlap = speed.range.intersect(&part).unwrap_or(part);
            if overlap.start() > at {
                push_piece(&mut out, at, overlap.start(), 1.0);
            }
            push_piece(&mut out, overlap.start(), overlap.end(), speed.factor);
            at = overlap.end();
        }
        push_piece(&mut out, at, part.end(), 1.0);
    }
    out
}

fn push_range(list: &mut Vec<Range<Source>>, start: SourceTime, end: SourceTime) {
    if let Ok(range) = Range::new(start, end)
        && !range.is_empty()
    {
        list.push(range);
    }
}

fn push_piece(
    list: &mut Vec<(Range<Source>, f64)>,
    start: SourceTime,
    end: SourceTime,
    speed: f64,
) {
    if let Ok(range) = Range::new(start, end)
        && !range.is_empty()
    {
        list.push((range, speed));
    }
}

#[cfg(test)]
#[path = "timemap_tests.rs"]
mod tests;
