use dc_core::History;

use super::*;
use crate::test_util::{duration, or, project, speed as sp, sr};

fn run(p: &mut Project, range: Range<Output>, factor: f64) -> Result<String, CommandError> {
    speed(p, range, factor).map(|diff| diff.to_string())
}

#[test]
fn speed_up_part_of_a_clip() {
    let mut p = project(&[10.0]);
    let diff = run(&mut p, or(2.0, 6.0), 2.0).unwrap();
    assert_eq!(p.clips[0].speeds, vec![sp(2.0, 6.0, 2.0)]);
    assert_eq!(diff, "скорость 2× 0:02.000–0:06.000; клипы ~1 c1");
    assert!((duration(&p) - 8.0).abs() < 1e-6);
}

#[test]
fn new_speed_replaces_the_overlapped_part_only() {
    let mut p = project(&[10.0]);
    p.clips[0].speeds = vec![sp(2.0, 8.0, 2.0)];
    // Output 3–4 is source 4–6 at 2×.
    run(&mut p, or(3.0, 4.0), 0.5).unwrap();
    assert_eq!(
        p.clips[0].speeds,
        vec![sp(2.0, 4.0, 2.0), sp(4.0, 6.0, 0.5), sp(6.0, 8.0, 2.0)]
    );
}

#[test]
fn speed_1_resets_and_equal_neighbours_merge() {
    let mut p = project(&[10.0]);
    run(&mut p, or(2.0, 4.0), 2.0).unwrap();
    // Output 3–5 is now source 4–6: touches source 2–4 at 2×.
    run(&mut p, or(3.0, 5.0), 2.0).unwrap();
    assert_eq!(p.clips[0].speeds, vec![sp(2.0, 6.0, 2.0)]);
    // Output 0–3 is source 0–4.
    run(&mut p, or(0.0, 3.0), 1.0).unwrap();
    assert_eq!(p.clips[0].speeds, vec![sp(4.0, 6.0, 2.0)]);
    assert_eq!(run(&mut p, or(0.0, 1.0), 1.0).unwrap(), "");
}

#[test]
fn speed_across_a_joint_and_a_cut() {
    let mut p = project(&[4.0, 4.0]);
    p.clips[0].cuts = vec![sr(1.0, 2.0)];
    run(&mut p, or(0.5, 4.0), 4.0).unwrap();
    assert_eq!(p.clips[0].speeds, vec![sp(0.5, 4.0, 4.0)]);
    assert_eq!(p.clips[1].speeds, vec![sp(0.0, 1.0, 4.0)]);
}

#[test]
fn extremes_are_allowed_and_outside_is_refused() {
    let mut p = project(&[10.0]);
    run(&mut p, or(0.0, 1.0), 0.25).unwrap();
    run(&mut p, or(4.0, 10.0), 16.0).unwrap();
    assert!((duration(&p) - (4.0 + 3.0 + 0.375)).abs() < 1e-6);
    for bad in [0.2, 16.5, f64::NAN, f64::INFINITY, 0.0, -2.0] {
        let error = run(&mut p, or(0.0, 1.0), bad).unwrap_err();
        assert!(error.to_string().contains("0.25–16"), "{error}");
    }
}

#[test]
fn undo_then_redo_returns_the_same_project() {
    let mut p = project(&[10.0]);
    let mut history = History::new(10);
    let command: Speed =
        serde_json::from_value(serde_json::json!({"range": "1-3", "factor": 3})).unwrap();
    history.apply(&mut p, &command).unwrap();
    let after = p.clone();
    history.undo(&mut p);
    assert_eq!(p, project(&[10.0]));
    history.redo(&mut p);
    assert_eq!(p, after);
}
