use dc_core::project::{Media, MediaKind, SpeedRange};
use dc_core::time::{Output, Source};
use dc_core::{Clip, ClipId, OutputTime, Project, Range, SourceTime, TimeDelta, TimeMap};

pub fn s(secs: f64) -> SourceTime {
    SourceTime::from_secs_f64(secs)
}

pub fn o(secs: f64) -> OutputTime {
    OutputTime::from_secs_f64(secs)
}

pub fn sr(a: f64, b: f64) -> Range<Source> {
    Range::new(s(a), s(b)).unwrap()
}

pub fn or(a: f64, b: f64) -> Range<Output> {
    Range::new(o(a), o(b)).unwrap()
}

pub fn speed(a: f64, b: f64, factor: f64) -> SpeedRange {
    SpeedRange {
        range: sr(a, b),
        factor,
    }
}

/// A project with one clip per duration, each trimmed to its whole source.
pub fn project(durations: &[f64]) -> Project {
    let mut p = Project::default();
    for &duration in durations {
        let id = p.clip_ids.allocate();
        p.clips.push(Clip {
            id,
            media: Media {
                path: format!("media/{id}.mp4"),
                kind: MediaKind::Recording,
                duration: TimeDelta::from_secs_f64(duration),
                size: dc_core::PixelSize {
                    width: 1920,
                    height: 1080,
                },
            },
            trim: sr(0.0, duration),
            cuts: Vec::new(),
            speeds: Vec::new(),
            crop: dc_core::Rect::FULL,
        });
    }
    p
}

pub fn c(n: u32) -> ClipId {
    ClipId::from_number(n)
}

pub fn duration(p: &Project) -> f64 {
    TimeMap::build(p).duration().as_secs_f64()
}

pub fn order(p: &Project) -> Vec<u32> {
    p.clips.iter().map(|clip| clip.id.number()).collect()
}
