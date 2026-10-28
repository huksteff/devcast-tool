use proptest::prelude::*;

use super::*;
use crate::project::{Media, MediaKind, SpeedRange};

fn t(secs: f64) -> SourceTime {
    SourceTime::from_secs_f64(secs)
}

fn r(a: f64, b: f64) -> Range<Source> {
    Range::new(t(a), t(b)).unwrap()
}

fn add_clip(
    p: &mut Project,
    duration: f64,
    cuts: Vec<Range<Source>>,
    speeds: Vec<SpeedRange>,
) -> ClipId {
    let id = p.clip_ids.allocate();
    p.clips.push(Clip {
        id,
        media: Media {
            path: format!("media/{id}.mp4"),
            kind: MediaKind::Recording,
            duration: TimeDelta::from_secs_f64(duration),
            size: crate::geom::PixelSize {
                width: 1920,
                height: 1080,
            },
        },
        trim: r(0.0, duration),
        cuts,
        speeds,
        crop: crate::geom::Rect::FULL,
    });
    id
}

fn o(secs: f64) -> OutputTime {
    OutputTime::from_secs_f64(secs)
}

#[test]
fn cut_removes_time_and_shifts_later_content() {
    let mut p = Project::default();
    let c = add_clip(&mut p, 10.0, vec![r(2.0, 5.0)], vec![]);
    let map = TimeMap::build(&p);
    assert_eq!(map.duration(), TimeDelta::from_secs_f64(7.0));
    assert_eq!(map.to_output(c, t(1.0)), Some(o(1.0)));
    assert_eq!(
        map.to_output(c, t(3.0)),
        None,
        "cut moment is not in the output"
    );
    assert_eq!(map.to_output(c, t(6.0)), Some(o(3.0)));
    assert_eq!(map.to_source(o(3.0)), Some((c, t(6.0))));
}

#[test]
fn speed_range_shortens_output() {
    let mut p = Project::default();
    let c = add_clip(
        &mut p,
        10.0,
        vec![],
        vec![SpeedRange {
            range: r(2.0, 6.0),
            factor: 4.0,
        }],
    );
    let map = TimeMap::build(&p);
    assert_eq!(map.duration(), TimeDelta::from_secs_f64(7.0));
    assert_eq!(map.to_output(c, t(6.0)), Some(o(3.0)));
    assert_eq!(map.to_source(o(2.5)), Some((c, t(4.0))));
}

#[test]
fn clips_follow_each_other_without_gaps() {
    let mut p = Project::default();
    let a = add_clip(&mut p, 4.0, vec![], vec![]);
    let b = add_clip(&mut p, 3.0, vec![r(0.0, 1.0)], vec![]);
    let map = TimeMap::build(&p);
    assert_eq!(map.duration(), TimeDelta::from_secs_f64(6.0));
    assert_eq!(
        map.to_source(o(3.999_999)),
        Some((a, SourceTime::from_micros(3_999_999)))
    );
    assert_eq!(map.to_source(o(4.0)), Some((b, t(1.0))));
    assert_eq!(map.to_source(o(6.0)), None, "end is exclusive");
    assert_eq!(map.to_source(OutputTime::from_micros(-1)), None);
}

#[test]
fn empty_project_has_zero_duration() {
    let map = TimeMap::build(&Project::default());
    assert_eq!(map.duration(), TimeDelta::ZERO);
    assert_eq!(map.to_source(OutputTime::ZERO), None);
}

/// A random valid clip: consecutive pieces of kind 0 (keep 1×), 1 (cut) or 2 (keep at a speed).
fn clip_strategy() -> impl Strategy<Value = (f64, Vec<Range<Source>>, Vec<SpeedRange>)> {
    prop::collection::vec(
        (
            1i64..2_000_000,
            0u8..3,
            prop::sample::select(vec![0.25, 0.5, 1.5, 2.0, 4.0, 16.0]),
        ),
        1..12,
    )
    .prop_map(|parts| {
        let (mut at, mut cuts, mut speeds) = (0i64, Vec::new(), Vec::new());
        for (len, kind, factor) in parts {
            let range = Range::new(
                SourceTime::from_micros(at),
                SourceTime::from_micros(at + len),
            )
            .unwrap();
            match kind {
                1 => cuts.push(range),
                2 => speeds.push(SpeedRange { range, factor }),
                _ => {}
            }
            at += len;
        }
        (at as f64 / 1e6, cuts, speeds)
    })
}

proptest! {
    #[test]
    fn segments_are_contiguous_and_sum_to_duration(clips in prop::collection::vec(clip_strategy(), 1..4)) {
        let mut p = Project::default();
        for (d, cuts, speeds) in clips {
            add_clip(&mut p, d, cuts, speeds);
        }
        prop_assert!(p.problems().is_empty(), "{:?}", p.problems());
        let map = TimeMap::build(&p);
        let mut expected = OutputTime::ZERO;
        for s in map.segments() {
            prop_assert_eq!(s.output_start, expected);
            expected = s.output_end();
        }
        prop_assert_eq!(expected.since(OutputTime::ZERO), map.duration());
    }

    #[test]
    fn source_output_round_trip(clip in clip_strategy(), frac in 0.0f64..1.0) {
        let (d, cuts, speeds) = clip;
        let mut p = Project::default();
        let id = add_clip(&mut p, d, cuts.clone(), speeds);
        let map = TimeMap::build(&p);
        prop_assume!(map.duration() > TimeDelta::ZERO);
        let out = OutputTime::from_micros((map.duration().micros() as f64 * frac) as i64);
        let (clip_id, src) = map.to_source(out).unwrap();
        prop_assert_eq!(clip_id, id);
        prop_assert!(!cuts.iter().any(|c| c.contains(src)), "mapped into a cut");
        let back = map.to_output(id, src).unwrap();
        // Source time has 1 µs resolution; in slow motion (speed < 1) one source µs spans
        // 1/speed output µs, so the round trip may land up to ceil(1/speed) µs away.
        let speed = map.segment_at(out).unwrap().speed;
        let tolerance = (1.0 / speed).ceil() as i64;
        prop_assert!((back.micros() - out.micros()).abs() <= tolerance, "{out} → {src} → {back} (speed {speed})");
    }

    #[test]
    fn to_output_is_monotonic_within_clip(clip in clip_strategy(), a in 0.0f64..1.0, b in 0.0f64..1.0) {
        let (d, cuts, speeds) = clip;
        let mut p = Project::default();
        let id = add_clip(&mut p, d, cuts, speeds);
        let map = TimeMap::build(&p);
        let (lo, hi) = (a.min(b), a.max(b));
        let ta = SourceTime::from_micros((d * 1e6 * lo) as i64);
        let tb = SourceTime::from_micros((d * 1e6 * hi) as i64);
        if let (Some(x), Some(y)) = (map.to_output(id, ta), map.to_output(id, tb)) {
            prop_assert!(x <= y);
        }
    }
}

#[test]
fn output_span_follows_the_recording_through_cuts_splits_and_order() {
    let mut p = Project::default();
    add_clip(&mut p, 10.0, vec![r(2.0, 3.0)], vec![]);
    let mut right = p.clips[0].clone();
    right.id = p.clip_ids.allocate();
    p.clips[0].trim = r(0.0, 5.0);
    p.clips[0].cuts = vec![r(2.0, 3.0)];
    right.trim = r(5.0, 10.0);
    right.cuts = Vec::new();
    add_clip(&mut p, 4.0, vec![], vec![]);
    p.clips.insert(1, right);
    let media = p.clips[0].media.path.clone();
    let map = TimeMap::build(&p);
    let span = |a, b| {
        map.output_span(&media, r(a, b))
            .map(|s| (s.start(), s.end()))
    };
    assert_eq!(
        span(1.0, 4.0),
        Some((o(1.0), o(3.0))),
        "cut inside is skipped"
    );
    assert_eq!(span(4.0, 6.0), Some((o(3.0), o(5.0))), "across the split");
    assert_eq!(span(2.0, 3.0), None, "all cut out");
    assert_eq!(map.output_span("media/none.mp4", r(0.0, 1.0)), None);
}
