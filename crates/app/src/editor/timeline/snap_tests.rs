//! Where dragged regions go: sticking to the nearest target, the video's ends, the neighbours
//! that must not be overlapped, the least length.

use dc_core::time::Output;
use dc_core::{OutputTime, Range, TimeDelta};

use super::snap::{Edge, Moving, Shift, edge, shift};

fn t(secs: f64) -> OutputTime {
    OutputTime::from_secs_f64(secs)
}

fn d(secs: f64) -> TimeDelta {
    TimeDelta::from_secs_f64(secs)
}

fn r(from: f64, to: f64) -> Range<Output> {
    Range::new(t(from), t(to)).unwrap()
}

/// 10 points per second: the 8-point snap reaches 0.8 s.
const PPS: f32 = 10.0;

#[test]
fn a_moved_group_sticks_by_its_nearest_edge() {
    let group = [
        Moving {
            range: r(2.0, 4.0),
            still: &[],
        },
        Moving {
            range: r(6.0, 7.0),
            still: &[],
        },
    ];
    let targets = [t(9.0), t(5.0)];
    let moved = shift(&group, d(1.7), &targets, PPS, t(100.0));
    assert_eq!(
        moved,
        Shift {
            delta: d(2.0),
            stuck: Some(t(9.0))
        },
        "7 + 1.7 = 8.7 is 0.3 from 9; 4 + 1.7 = 5.7 is 0.7 from 5"
    );
    let free = shift(&group, d(1.0), &targets, PPS, t(100.0));
    assert_eq!(
        free,
        Shift {
            delta: d(1.0),
            stuck: Some(t(5.0))
        },
        "4 + 1 = 5 exactly"
    );
    let none = shift(&group, d(1.5), &[], PPS, t(100.0));
    assert_eq!(
        none,
        Shift {
            delta: d(1.5),
            stuck: None
        },
        "«Привязка» off"
    );
    let far = shift(&group, d(1.5), &[t(30.0)], PPS, t(100.0));
    assert_eq!(far.stuck, None, "a target beyond 8 points lets go");
}

#[test]
fn a_moved_group_stays_in_the_video_and_off_still_neighbours() {
    let still = [r(0.0, 1.0), r(8.0, 9.0)];
    let group = [Moving {
        range: r(3.0, 5.0),
        still: &still,
    }];
    assert_eq!(shift(&group, d(-5.0), &[], PPS, t(20.0)).delta, d(-2.0));
    assert_eq!(shift(&group, d(10.0), &[], PPS, t(20.0)).delta, d(3.0));
    let open = [Moving {
        range: r(3.0, 5.0),
        still: &[],
    }];
    assert_eq!(
        shift(&open, d(-5.0), &[], PPS, t(20.0)).delta,
        d(-3.0),
        "zero"
    );
    assert_eq!(
        shift(&open, d(30.0), &[], PPS, t(20.0)).delta,
        d(15.0),
        "the end"
    );
    let pushed = shift(&group, d(3.6), &[t(9.5)], PPS, t(20.0));
    assert_eq!(
        pushed,
        Shift {
            delta: d(3.0),
            stuck: None
        },
        "stopped short of the target: no yellow line"
    );
}

#[test]
fn an_edge_sticks_and_keeps_the_least_length_and_the_neighbours() {
    let range = r(4.0, 6.0);
    let still = [r(1.0, 3.0), r(7.0, 8.0)];
    let video = t(20.0);
    let stuck = edge(range, true, t(6.6), &still, &[t(6.5)], PPS, video);
    assert_eq!(
        stuck,
        Edge {
            at: t(6.5),
            stuck: Some(t(6.5))
        }
    );
    assert_eq!(
        edge(range, true, t(9.0), &still, &[], PPS, video).at,
        t(7.0)
    );
    assert_eq!(
        edge(range, true, t(1.0), &still, &[], PPS, video).at,
        t(4.1),
        "0.1 s at least"
    );
    assert_eq!(
        edge(range, false, t(0.5), &still, &[], PPS, video).at,
        t(3.0)
    );
    assert_eq!(
        edge(range, false, t(9.0), &still, &[], PPS, video).at,
        t(5.9)
    );
    assert_eq!(edge(range, false, t(-1.0), &[], &[], PPS, video).at, t(0.0));
    assert_eq!(edge(range, true, t(30.0), &[], &[], PPS, video).at, video);
}
