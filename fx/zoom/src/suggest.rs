//! Auto zooms (docs/porting/zoom.md §10.4): presses close in time on the output become one zoom
//! from just before the first to just after the last. Pauses of the pointer are not used
//! (decision 000). The CLI reads the telemetry and passes it in; [`suggest`] only proposes,
//! [`Suggestion::command`] makes the `zoom add` that places one. [`by_clicks`] asks for all of
//! them as a step (`zoom add --at clicks`), which `dc_ops::autozoom` resolves.

use dc_core::OpName;
use dc_core::TimeMap;
use dc_core::time::{Output, Source};
use dc_core::timemap::Segment;
use dc_core::{
    CommandError, OutputTime, Point, Project, Range, SourceTime, Step, TimeArg, TimeDelta,
};
use serde_json::Value;

use crate::classify::strength;
use crate::model::{Mode, ZoomEffect};
use crate::region::ZoomAdd;
use crate::span::{self, timeline};

/// Presses at most this far apart on the output are one cluster.
pub const CLUSTER_GAP: TimeDelta = TimeDelta::from_millis(2500);
/// A cluster's zoom starts this long before its first press and ends this long after the last.
pub const CLUSTER_PAD: TimeDelta = TimeDelta::from_millis(500);
/// Presses this close to the end of a recording are the click that stopped it (the HUD's
/// «Стоп», a shortcut): never a zoom.
pub const STOP_TAIL: TimeDelta = TimeDelta::from_millis(1000);
/// The value of `zoom add --at` that places the suggested zooms. A command is a pure function of
/// the project and the clicks lie in the recordings' files, so `dc_ops::autozoom` resolves it.
pub const AT_CLICKS: &str = "clicks";

/// `zoom add --at clicks`: the suggested zooms (scale 1.3, follow) as one step, as `devcast` and
/// the plans of `devcast apply` give it.
pub fn by_clicks() -> Step {
    let args = [("at".to_owned(), Value::from(AT_CLICKS))]
        .into_iter()
        .collect();
    Step::of(OpName::of::<ZoomAdd>(), args)
}

/// The pointer of one recording (`Media.path`), in source time order.
#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    pub media: String,
    pub pointer: Vec<Pointer>,
}

/// A pointer event; `point` is a fraction of the recorded frame (before the clip's crop).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    pub at: SourceTime,
    pub point: Point,
    pub action: Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Move,
    /// A left, right or middle button went down; `double` — the second press of a double click.
    Press {
        double: bool,
    },
    Release,
}

/// A proposed zoom: where it plays and where it looks (a point of the clip's picture after crop).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Suggestion {
    pub span: Range<Output>,
    pub focus: Point,
    pub clicks: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Suggestions {
    pub zooms: Vec<Suggestion>,
    /// Presses seen in the video.
    pub clicks: usize,
    /// Presses cut out or outside the clip's picture.
    pub hidden: usize,
    /// Clusters skipped because a zoom is already there.
    pub taken: usize,
}

/// A press placed on the output.
struct Press {
    at: OutputTime,
    run: usize,
    focus: Point,
    strength: u32,
}

impl Suggestion {
    pub fn command(&self, scale: f64, mode: Mode) -> ZoomAdd {
        ZoomAdd {
            at: TimeArg(self.span.start()),
            duration: self.span.duration().as_secs_f64(),
            scale,
            mode,
            x: self.focus.x,
            y: self.focus.y,
        }
    }
}

/// Zooms for the clusters of presses of `recordings` that do not overlap the project's zooms
/// or each other, in output order.
pub fn suggest(project: &Project, recordings: &[Recording]) -> Result<Suggestions, CommandError> {
    let timeline = timeline(project)?;
    let map = TimeMap::build(project);
    let runs = runs(map.segments());
    let mut found = Suggestions::default();
    let mut presses = Vec::new();
    for recording in recordings {
        let stopping = stop_tail(project, &recording.media);
        for (index, p) in recording.pointer.iter().enumerate() {
            if !matches!(p.action, Action::Press { .. }) || stopping.is_some_and(|s| p.at >= s) {
                continue;
            }
            match place(project, &map, &runs, &recording.media, p) {
                Some((at, run, focus)) => presses.push(Press {
                    at,
                    run,
                    focus,
                    strength: strength(&recording.pointer, index),
                }),
                None => found.hidden += 1,
            }
        }
    }
    presses.sort_by_key(|p| p.at);
    found.clicks = presses.len();
    let mut reserved: Vec<(String, Range<Source>)> = project
        .state::<ZoomEffect>()?
        .zooms
        .into_iter()
        .map(|z| (z.media, z.range))
        .collect();
    for cluster in clusters(&presses) {
        let (first, last) = (&cluster[0], &cluster[cluster.len() - 1]);
        let (run_start, run_end) = runs[first.run];
        let start = (first.at - CLUSTER_PAD).max(run_start);
        let end = (last.at + CLUSTER_PAD).min(run_end);
        let span = Range::new(start, end).map_err(|e| CommandError::new(e.to_string()))?;
        let source = span::span(&timeline, timeline.start(start)?, timeline.end(end)?)?;
        let overlaps = |(media, range): &(String, Range<Source>)| {
            *media == source.0 && range.intersect(&source.1).is_some()
        };
        if reserved.iter().any(overlaps) {
            found.taken += 1;
            continue;
        }
        reserved.push(source);
        found.zooms.push(Suggestion {
            span,
            focus: strongest(cluster),
            clicks: cluster.len(),
        });
    }
    Ok(found)
}

/// Where the last [`STOP_TAIL`] of the recording `media` begins; none for a media not in the
/// project.
fn stop_tail(project: &Project, media: &str) -> Option<SourceTime> {
    let length = project
        .clips
        .iter()
        .find(|c| c.media.path == media)?
        .media
        .duration;
    let end = SourceTime::ZERO + length;
    Some(end.checked_sub(STOP_TAIL)?.max(SourceTime::ZERO))
}

/// Output range of each clip on the track. An auto zoom stays within one clip: there its source
/// time only goes forward, so the zoom covers no part of the recording shown elsewhere.
fn runs(segments: &[Segment]) -> Vec<(OutputTime, OutputTime)> {
    let mut runs: Vec<(OutputTime, OutputTime)> = Vec::new();
    for (i, s) in segments.iter().enumerate() {
        match runs.last_mut() {
            Some(run) if i > 0 && segments[i - 1].clip == s.clip => run.1 = s.output_end(),
            _ => runs.push((s.output_start, s.output_end())),
        }
    }
    runs
}

/// Output time, clip run and focus of press `p` of `media`; `None` if it is cut out or outside the
/// picture of its clip.
fn place(
    project: &Project,
    map: &TimeMap,
    runs: &[(OutputTime, OutputTime)],
    media: &str,
    p: &Pointer,
) -> Option<(OutputTime, usize, Point)> {
    let (segment, at) = map.locate(media, p.at)?;
    let clip = project.clips.iter().find(|c| c.id == segment.clip)?;
    let focus = clip.crop.to_local(p.point);
    let inside = |v: f64| (0.0..=1.0).contains(&v);
    let run = runs
        .iter()
        .position(|&(start, end)| start <= at && at < end)?;
    (inside(focus.x) && inside(focus.y)).then_some((at, run, focus))
}

/// Focus of the strongest press of `cluster`; of equally strong ones, the earliest.
fn strongest(cluster: &[Press]) -> Point {
    let mut best = &cluster[0];
    for press in cluster {
        if press.strength > best.strength {
            best = press;
        }
    }
    best.focus
}

/// Consecutive presses in one clip at most [`CLUSTER_GAP`] apart.
fn clusters(presses: &[Press]) -> Vec<&[Press]> {
    let mut clusters = Vec::new();
    let mut start = 0;
    for i in 1..=presses.len() {
        let split = presses.get(i).is_none_or(|p| {
            let previous = &presses[i - 1];
            p.run != previous.run || p.at.since(previous.at) > CLUSTER_GAP
        });
        if split {
            clusters.push(&presses[start..i]);
            start = i;
        }
    }
    clusters
}
