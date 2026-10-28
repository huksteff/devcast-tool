use dc_core::{History, Op};
use serde_json::json;

use super::*;
use crate::test_util::{c, duration, order, project};

fn parse<T: Op>(args: serde_json::Value) -> T {
    serde_json::from_value(args).unwrap()
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

fn imported(path: &str, secs: f64) -> ClipAdd {
    ClipAdd {
        media: Media {
            path: path.to_owned(),
            kind: dc_core::project::MediaKind::Imported,
            duration: dc_core::TimeDelta::from_secs_f64(secs),
            size: dc_core::PixelSize {
                width: 640,
                height: 360,
            },
        },
    }
}

#[test]
fn add_puts_the_whole_video_at_the_end_and_undo_removes_it() {
    let mut p = project(&[1.0, 2.0]);
    let mut history = History::new(10);
    let diff = history
        .apply(&mut p, &imported("media/урок 1.mp4", 2.5))
        .unwrap();
    assert_eq!(diff.to_string(), "клипы +1 c3");
    assert_eq!(order(&p), vec![1, 2, 3]);
    let clip = &p.clips[2];
    assert_eq!(clip.trim.duration(), clip.media.duration);
    assert_eq!(clip.crop, Rect::FULL);
    assert!((duration(&p) - 5.5).abs() < 1e-6);
    let after = p.clone();
    history.undo(&mut p).unwrap();
    assert_eq!(order(&p), vec![1, 2]);
    history.redo(&mut p).unwrap();
    assert_eq!(p, after);
}

#[test]
fn add_to_an_empty_project_gives_c1_and_empty_video_is_refused() {
    let mut p = Project::default();
    let mut history = History::new(10);
    history
        .apply(&mut p, &imported("media/a.mkv", 1.0))
        .unwrap();
    assert_eq!(order(&p), vec![1]);
    let error = history
        .apply(&mut p, &imported("media/b.mkv", 0.0))
        .unwrap_err();
    assert!(error.to_string().contains("пустой trim"), "{error}");
    assert_eq!(order(&p), vec![1]);
}
