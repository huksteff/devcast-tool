use dc_core::{History, Op};
use serde_json::json;

use super::*;
use crate::test_util::{c, duration, o, order, project, speed, sr};

fn parse<T: Op>(args: serde_json::Value) -> T {
    serde_json::from_value(args).unwrap()
}

#[test]
fn split_divides_trim_cuts_and_speeds() {
    let mut p = project(&[10.0, 2.0]);
    p.clips[0].cuts = vec![sr(1.0, 2.0), sr(5.0, 7.0)];
    p.clips[0].speeds = vec![speed(3.0, 6.0, 2.0)];
    // Output 0–1 is source 0–1, 1–2 is 2–3, 2–3 is 3–5 at 2×: output 2.5 is source 4.
    let diff = split(&mut p, o(2.5)).unwrap();
    assert_eq!(diff.to_string(), "клипы ~1 c1; клипы +1 c3");
    assert_eq!(order(&p), vec![1, 3, 2]);
    let (left, right) = (&p.clips[0], &p.clips[1]);
    assert_eq!((left.trim, right.trim), (sr(0.0, 4.0), sr(4.0, 10.0)));
    assert_eq!(left.cuts, vec![sr(1.0, 2.0)]);
    assert_eq!(right.cuts, vec![sr(5.0, 7.0)]);
    assert_eq!(left.speeds, vec![speed(3.0, 4.0, 2.0)]);
    assert_eq!(right.speeds, vec![speed(4.0, 6.0, 2.0)]);
    assert_eq!(right.media, left.media);
    assert!((duration(&p) - 8.0).abs() < 1e-6);
}

#[test]
fn split_at_a_joint_or_the_ends_is_refused() {
    let mut p = project(&[4.0, 4.0]);
    for at in [0.0, 4.0, 8.0, 9.0] {
        let error = split(&mut p, o(at)).unwrap_err();
        assert!(error.to_string().contains("разрез"), "{at}: {error}");
    }
    assert_eq!(order(&p), vec![1, 2]);
}

#[test]
fn split_where_only_cut_material_is_on_the_left_is_refused() {
    let mut p = project(&[10.0]);
    p.clips[0].cuts = vec![sr(0.0, 2.0)];
    assert!(split(&mut p, o(0.0)).is_err());
}

#[test]
fn split_right_after_a_cut_keeps_both_halves() {
    let mut p = project(&[10.0]);
    p.clips[0].cuts = vec![sr(2.0, 4.0)];
    split(&mut p, o(2.0)).unwrap();
    assert_eq!(p.clips[0].trim, sr(0.0, 4.0));
    assert_eq!(p.clips[0].cuts, vec![sr(2.0, 4.0)]);
    assert_eq!(p.clips[1].trim, sr(4.0, 10.0));
}

#[test]
fn split_undo_redo() {
    let mut p = project(&[10.0]);
    let mut history = History::new(10);
    let command: ClipSplit = parse(json!({"at": "0:04"}));
    history.apply(&mut p, &command).unwrap();
    let after = p.clone();
    history.undo(&mut p);
    assert_eq!(p.clips, project(&[10.0]).clips);
    history.redo(&mut p);
    assert_eq!(p, after);
}

#[test]
fn rm_removes_and_closes_the_gap() {
    let mut p = project(&[1.0, 2.0, 3.0]);
    let rm: ClipRm = parse(json!({"id": "c2"}));
    let diff = History::new(10).apply(&mut p, &rm).unwrap();
    assert_eq!(diff.to_string(), "клипы -1 c2");
    assert_eq!(order(&p), vec![1, 3]);
    assert!((duration(&p) - 4.0).abs() < 1e-6);
}

#[test]
fn rm_of_missing_or_last_clip_is_refused() {
    let mut p = project(&[1.0, 2.0]);
    let missing = parse::<ClipRm>(json!({"id": "c9"}))
        .apply(&mut p)
        .unwrap_err();
    assert_eq!(
        missing.to_string(),
        "c9 не найден (есть c1,c2); список: devcast ls clips"
    );
    parse::<ClipRm>(json!({"id": "c1"})).apply(&mut p).unwrap();
    let last = parse::<ClipRm>(json!({"id": "c2"}))
        .apply(&mut p)
        .unwrap_err();
    assert!(last.to_string().contains("последний"), "{last}");
}

#[test]
fn move_reorders_by_position_from_one() {
    let mut p = project(&[1.0, 2.0, 3.0]);
    let diff = parse::<ClipMove>(json!({"id": "c3", "to": 1}))
        .apply(&mut p)
        .unwrap();
    assert_eq!(diff.to_string(), "клипы ~1 c3");
    assert_eq!(order(&p), vec![3, 1, 2]);
    parse::<ClipMove>(json!({"id": "c3", "to": 3}))
        .apply(&mut p)
        .unwrap();
    assert_eq!(order(&p), vec![1, 2, 3]);
    let same = parse::<ClipMove>(json!({"id": "c2", "to": 2}));
    assert!(same.apply(&mut p).unwrap().is_empty());
}

#[test]
fn move_outside_the_track_or_missing_is_refused() {
    let mut p = project(&[1.0, 2.0]);
    for to in [0, 3] {
        let error = parse::<ClipMove>(json!({"id": "c1", "to": to}))
            .apply(&mut p)
            .unwrap_err();
        assert!(error.to_string().contains("1–2"), "{error}");
    }
    let missing = parse::<ClipMove>(json!({"id": "c7", "to": 1}))
        .apply(&mut p)
        .unwrap_err();
    assert!(missing.to_string().contains("не найден"), "{missing}");
    assert_eq!(p.clip(c(1)).map(|clip| clip.id), Some(c(1)));
}
