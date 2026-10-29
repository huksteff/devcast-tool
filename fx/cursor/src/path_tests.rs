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
    let path = Path::new(jump(), None, TimeDelta::from_secs_f64(3.0));
    assert_eq!(path.at(at(0.0)).x, 0.1);
    assert_eq!(path.at(at(0.999)).x, 0.1);
    assert_eq!(path.at(at(1.0)).x, 0.9);
    assert_eq!(path.at(at(10.0)).x, 0.9);
}

#[test]
fn smoothed_cursor_follows_with_a_lag_and_arrives() {
    let spring = Smoothing::Focused.spring().unwrap();
    let path = Path::new(jump(), Some(&spring), TimeDelta::from_secs_f64(3.0));
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
    let path = Path::new(jump(), Some(&spring), TimeDelta::from_secs_f64(2.0));
    let mut last = path.at(at(1.0)).x;
    for micros in (1_000_000..1_300_000).step_by(997) {
        let x = path.at(SourceTime::from_micros(micros)).x;
        assert!(x >= last && x - last < 0.01, "{micros}: {last} → {x}");
        last = x;
    }
}
