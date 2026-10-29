use super::*;
use crate::state::Smoothing;

fn point(secs: f64, x: f64, y: f64) -> CursorPoint {
    CursorPoint {
        at: SourceTime::from_secs_f64(secs),
        pos: Point { x, y },
    }
}

fn at(secs: f64) -> SourceTime {
    SourceTime::from_secs_f64(secs)
}

/// The pointer rests at the left, jumps to the right at 1 s and stays there.
fn jump() -> Vec<CursorPoint> {
    vec![point(0.5, 0.1, 0.5), point(1.0, 0.9, 0.5)]
}

#[test]
fn exact_motion_holds_each_event_until_the_next() {
    let path = Path::new(jump(), None, None, TimeDelta::from_secs_f64(3.0), 0.5625);
    assert_eq!(path.at(at(0.0)).x, 0.1);
    assert_eq!(path.at(at(0.999)).x, 0.1);
    assert_eq!(path.at(at(1.0)).x, 0.9);
    assert_eq!(path.at(at(10.0)).x, 0.9);
}

#[test]
fn smoothed_cursor_follows_with_a_lag_and_arrives() {
    let spring = Smoothing::Focused.spring().unwrap();
    let path = Path::new(
        jump(),
        Some(&spring),
        None,
        TimeDelta::from_secs_f64(3.0),
        0.5625,
    );
    assert_eq!(path.at(at(0.0)).x, 0.1);
    assert_eq!(path.at(at(1.0)).x, 0.1);
    let moving = path.at(at(1.05)).x;
    assert!(moving > 0.1 && moving < 0.9, "{moving}");
    assert!((path.at(at(2.5)).x - 0.9).abs() < 1e-3);
    assert_eq!(path.at(at(2.5)).y, 0.5);
    // After the end of the recording it stays where the spring stopped.
    assert_eq!(path.at(at(9.0)), path.at(at(3.0)));
}

#[test]
fn smoothed_path_is_continuous_between_steps() {
    let spring = Smoothing::Smooth.spring().unwrap();
    let path = Path::new(
        jump(),
        Some(&spring),
        None,
        TimeDelta::from_secs_f64(2.0),
        0.5625,
    );
    let mut last = path.at(at(1.0)).x;
    for micros in (1_000_000..1_300_000).step_by(997) {
        let x = path.at(SourceTime::from_micros(micros)).x;
        assert!(x >= last && x - last < 0.01, "{micros}: {last} → {x}");
        last = x;
    }
}

fn sway(strength: f64) -> Sway {
    Sway {
        spring: Smoothing::Focused.sway_spring(),
        strength,
    }
}

/// The pointer sweeps right over 0.5–1.0 s (0.6 of the width), then stays.
fn sweep() -> Vec<CursorPoint> {
    (0..=60)
        .map(|i| {
            let q = f64::from(i) / 60.0;
            point(0.5 + 0.5 * q, 0.2 + 0.6 * q, 0.5)
        })
        .collect()
}

#[test]
fn sway_leans_into_the_motion_and_settles_upright() {
    let spring = Smoothing::Focused.spring().unwrap();
    let length = TimeDelta::from_secs_f64(3.0);
    let path = Path::new(sweep(), Some(&spring), Some(&sway(0.4)), length, 0.5625);
    assert_eq!(path.turn(at(0.4)), 0.0);
    // 2300 ref-px/s is past full speed: the lean tends to 0.4 · 30° = 12°.
    let lean = path.turn(at(0.9)).to_degrees();
    assert!(lean > 8.0 && lean < 14.0, "{lean}");
    assert_eq!(path.turn(at(2.5)), 0.0);
    let left: Vec<_> = sweep()
        .into_iter()
        .map(|p| point(p.at.as_secs_f64(), 1.0 - p.pos.x, 0.5))
        .collect();
    let back = Path::new(left, Some(&spring), Some(&sway(0.4)), length, 0.5625);
    assert!((back.turn(at(0.9)) + path.turn(at(0.9))).abs() < 1e-9);
}

#[test]
fn sway_does_not_move_the_cursor_and_works_with_exact_motion() {
    let length = TimeDelta::from_secs_f64(3.0);
    let exact = Path::new(sweep(), None, Some(&sway(2.0)), length, 0.5625);
    assert_eq!(exact.at(at(0.75)), exact.recorded(at(0.75)));
    let lean = exact.turn(at(0.9)).to_degrees();
    assert!(lean > 30.0 && lean < 70.0, "{lean}");
    let still = Path::new(sweep(), None, None, length, 0.5625);
    assert_eq!(still.turn(at(0.9)), 0.0);
}
