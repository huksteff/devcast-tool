use dc_core::{History, Rect};
use serde_json::json;

use super::*;
use crate::test_util::{c, duration, o, order, project, speed, sr};

fn split(at: f64) -> ClipSplit {
    serde_json::from_value(json!({ "at": at })).unwrap()
}

#[test]
fn halves_keep_settings_and_play_the_same() {
    let mut p = project(&[10.0, 4.0]);
    p.clips[0].cuts = vec![sr(2.0, 3.0), sr(7.0, 8.0)];
    p.clips[0].speeds = vec![speed(4.0, 6.0, 2.0)];
    let crop = Rect::new(0.1, 0.0, 0.8, 1.0).unwrap();
    p.clips[0].crop = crop;
    let before = TimeMap::build(&p);
    let mut history = History::new(10);
    let diff = history.apply(&mut p, &split(3.5)).unwrap();
    assert_eq!(diff.to_string(), "клипы ~1 c1; клипы +1 c3");
    assert_eq!(order(&p), vec![1, 3, 2]);
    let (left, right) = (&p.clips[0], &p.clips[1]);
    assert_eq!(left.trim, sr(0.0, 5.0));
    assert_eq!(right.trim, sr(5.0, 10.0));
    assert_eq!(left.cuts, vec![sr(2.0, 3.0)]);
    assert_eq!(right.cuts, vec![sr(7.0, 8.0)]);
    assert_eq!(left.speeds, vec![speed(4.0, 5.0, 2.0)]);
    assert_eq!(right.speeds, vec![speed(5.0, 6.0, 2.0)]);
    assert_eq!((left.crop, right.crop), (crop, crop));
    assert_eq!(right.media, left.media);
    let after = TimeMap::build(&p);
    assert_eq!(after.duration(), before.duration());
    for t in [0.5, 3.4, 3.5, 3.6, 6.0, 7.9] {
        let shown = |map: &TimeMap| map.to_source(o(t)).map(|(_, s)| s);
        assert_eq!(shown(&after), shown(&before), "same frame at {t}");
    }
    history.undo(&mut p);
    assert_eq!(order(&p), vec![1, 2]);
    history.redo(&mut p);
    assert_eq!(order(&p), vec![1, 3, 2]);
}

#[test]
fn split_outside_video_or_on_a_joint_is_refused() {
    let mut p = project(&[5.0, 5.0]);
    for (at, expected) in [
        (0.0, "внутри видео"),
        (10.0, "внутри видео"),
        (-1.0, "внутри видео"),
        (5.0, "уже стык"),
    ] {
        let error = split(at).apply(&mut p).unwrap_err().to_string();
        assert!(error.contains(expected), "{at}: {error}");
    }
    assert!((duration(&p) - 10.0).abs() < 1e-6);
    assert_eq!(p.clips[1].id, c(2));
}
