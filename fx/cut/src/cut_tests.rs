use dc_core::{Command, History};
use serde_json::json;

use super::*;
use crate::test_util::{c, duration, o, or, order, project, speed, sr};

fn run(p: &mut Project, ranges: &[Range<Output>]) -> Result<String, CommandError> {
    cut(p, ranges).map(|diff| diff.to_string())
}

#[test]
fn cut_inside_one_clip_adds_a_source_cut() {
    let mut p = project(&[10.0]);
    let diff = run(&mut p, &[or(2.0, 4.0)]).unwrap();
    assert_eq!(p.clips[0].cuts, vec![sr(2.0, 4.0)]);
    assert_eq!(diff, "вырез 0:02.000–0:04.000; клипы ~1 c1");
    assert!((duration(&p) - 8.0).abs() < 1e-6);
}

#[test]
fn ranges_are_read_before_any_of_them_is_cut() {
    let mut p = project(&[10.0]);
    run(&mut p, &[or(6.0, 8.0), or(1.0, 2.0)]).unwrap();
    assert_eq!(p.clips[0].cuts, vec![sr(1.0, 2.0), sr(6.0, 8.0)]);
}

#[test]
fn overlapping_and_touching_cuts_merge() {
    let mut p = project(&[10.0]);
    run(&mut p, &[or(2.0, 4.0)]).unwrap();
    // Output 2–3 is now source 4–5: touches the existing cut.
    run(&mut p, &[or(2.0, 3.0)]).unwrap();
    assert_eq!(p.clips[0].cuts, vec![sr(2.0, 5.0)]);
    run(&mut p, &[or(0.5, 1.0), or(0.8, 2.5)]).unwrap();
    assert_eq!(p.clips[0].cuts, vec![sr(0.5, 5.5)]);
}

#[test]
fn cut_across_clip_joint_cuts_both_clips() {
    let mut p = project(&[4.0, 4.0]);
    let diff = run(&mut p, &[or(3.0, 5.0)]).unwrap();
    assert_eq!(p.clips[0].cuts, vec![sr(3.0, 4.0)]);
    assert_eq!(p.clips[1].cuts, vec![sr(0.0, 1.0)]);
    assert_eq!(diff, "вырез 0:03.000–0:05.000; клипы ~2 c1,c2");
}

#[test]
fn cut_spanning_an_old_cut_covers_the_gap_too() {
    let mut p = project(&[10.0]);
    p.clips[0].cuts = vec![sr(3.0, 5.0)];
    run(&mut p, &[or(2.0, 4.0)]).unwrap();
    assert_eq!(p.clips[0].cuts, vec![sr(2.0, 6.0)]);
}

#[test]
fn cut_on_fast_range_cuts_more_source() {
    let mut p = project(&[10.0]);
    p.clips[0].speeds = vec![speed(2.0, 6.0, 2.0)];
    // Output 2–4 plays source 2–6 at 2×.
    run(&mut p, &[or(2.0, 3.0)]).unwrap();
    assert_eq!(p.clips[0].cuts, vec![sr(2.0, 4.0)]);
}

#[test]
fn cutting_a_whole_clip_removes_it() {
    let mut p = project(&[2.0, 3.0, 2.0]);
    let diff = run(&mut p, &[or(2.0, 5.0)]).unwrap();
    assert_eq!(order(&p), vec![1, 3]);
    assert_eq!(diff, "вырез 0:02.000–0:05.000; клипы -1 c2");
}

#[test]
fn start_and_end_of_the_video_can_be_cut() {
    let mut p = project(&[10.0]);
    run(&mut p, &[or(0.0, 1.0), or(9.0, 10.0)]).unwrap();
    assert_eq!(p.clips[0].cuts, vec![sr(0.0, 1.0), sr(9.0, 10.0)]);
}

#[test]
fn cutting_everything_is_refused() {
    let mut p = project(&[2.0, 3.0]);
    let error = run(&mut p, &[or(0.0, 5.0)]).unwrap_err();
    assert!(error.to_string().contains("всё видео"), "{error}");
}

#[test]
fn empty_and_outside_ranges_are_refused() {
    let mut p = project(&[10.0]);
    let empty = run(&mut p, &[or(3.0, 3.0)]).unwrap_err();
    assert!(empty.to_string().contains("пустой"), "{empty}");
    let outside = run(&mut p, &[or(9.0, 11.0)]).unwrap_err();
    assert!(outside.to_string().contains("0:10.000"), "{outside}");
    let negative = run(&mut p, &[Range::new(o(-1.0), o(1.0)).unwrap()]).unwrap_err();
    assert!(negative.to_string().contains("выходит"), "{negative}");
    assert!(p.clips[0].cuts.is_empty());
}

#[test]
fn arguments_are_a_comma_separated_list() {
    let mut p = project(&[60.0, 60.0]);
    let command: Cut = serde_json::from_value(json!({"ranges": "1:02-1:05.5,10-20"})).unwrap();
    let diff = command.apply(&mut p).unwrap().to_string();
    assert_eq!(
        diff,
        "вырез 1:02.000–1:05.500; вырез 0:10.000–0:20.000; клипы ~2 c1,c2"
    );
    assert_eq!(p.clips[1].cuts, vec![sr(2.0, 5.5)]);
    assert_eq!(p.clips[0].cuts, vec![sr(10.0, 20.0)]);
    let typo = serde_json::from_value::<Cut>(json!({"ranges": "1-2", "rangse": "3-4"}));
    assert!(typo.is_err());
}

#[test]
fn undo_then_redo_returns_the_same_project() {
    let mut p = project(&[4.0, 4.0]);
    let mut history = History::new(10);
    let command: Cut = serde_json::from_value(json!({"ranges": "3-5"})).unwrap();
    history.apply(&mut p, &command).unwrap();
    let after = p.clone();
    history.undo(&mut p);
    assert_eq!(p, project(&[4.0, 4.0]));
    history.redo(&mut p);
    assert_eq!(p, after);
    assert_eq!(p.clip(c(2)).map(|clip| clip.cuts.len()), Some(1));
}
