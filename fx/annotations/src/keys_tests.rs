use dc_core::{Command, History, OutputTime, PixelSize, Project, Rect, TimeDelta, TimeMap};
use dc_render::Rgba;
use serde_json::{Value, json};

use crate::tests::{add, command, cut_and_fast, fail, project, range, run, state};
use crate::{
    AnnotKeyAdd, AnnotKeyCopy, AnnotKeyRm, AnnotMove, AnnotSet, Annotation, Blend, Placement, keyed,
};

fn ms(ms: i64) -> TimeDelta {
    TimeDelta::from_millis(ms)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn set(args: Value) -> AnnotSet {
    command(args)
}

fn key_add(args: Value) -> AnnotKeyAdd {
    command(args)
}

fn key_rm(args: Value) -> AnnotKeyRm {
    command(args)
}

fn key_copy(args: Value) -> AnnotKeyCopy {
    command(args)
}

/// Box and rotation of annotation `index` drawn at output second `t`, in fractions of a
/// 1000×1000 canvas.
fn drawn(p: &Project, index: usize, t: f64) -> (f64, f64, f32) {
    let map = TimeMap::build(p);
    let t = OutputTime::from_secs_f64(t);
    let now = map.segment_at(t).unwrap().source_at(t);
    let canvas = PixelSize {
        width: 1000,
        height: 1000,
    };
    let placed = Placement::at(&state(p).annotations[index], canvas, &map, now, t);
    let r = placed.rect;
    (
        f64::from(r.x) / 1000.0,
        f64::from(r.w) / 1000.0,
        placed.rotation,
    )
}

fn annotation(p: &Project, index: usize) -> Annotation {
    state(p).annotations[index].clone()
}

#[test]
fn keys_hold_at_the_ends_and_blend_with_smoothstep() {
    let keys = [(ms(1000), 0.0), (ms(3000), 100.0)];
    assert_eq!(
        keyed(&7.0, &[], ms(0)),
        7.0,
        "no keys: the annotation itself"
    );
    assert_eq!(keyed(&7.0, &keys, ms(0)), 0.0, "before the first key");
    assert!(close(keyed(&7.0, &keys, ms(1500)), 15.625), "v1 test");
    assert!(close(keyed(&7.0, &keys, ms(2000)), 50.0));
    assert_eq!(keyed(&7.0, &keys, ms(3000)), 100.0);
    assert_eq!(keyed(&7.0, &keys, ms(9000)), 100.0, "after the last key");
    let three = [(ms(0), 0.0), (ms(1000), 10.0), (ms(2000), 0.0)];
    assert_eq!(keyed(&0.0, &three, ms(1000)), 10.0, "on a key exactly");
}

#[test]
fn colours_blend_by_channel_and_the_rest_steps() {
    let black = Rgba([0, 0, 0, 255]);
    let white = Rgba([255, 255, 255, 0]);
    assert_eq!(black.blend(&white, 0.5), Rgba([128, 128, 128, 128]));
    assert_eq!("a".to_owned().blend(&"b".to_owned(), 0.99), "a");
    assert_eq!("a".to_owned().blend(&"b".to_owned(), 1.0), "b");
    let left = json!({"size": 32.0, "count": 1, "color": "#000000ff", "text": "#a", "on": true,
        "list": [1, 2], "only_left": 1});
    let right = json!({"size": 48.0, "count": 4, "color": "#ffffffff", "text": "#b", "on": false,
        "list": [3, 4]});
    let mid = left.blend(&right, 0.5);
    assert_eq!(
        mid,
        json!({"size": 40.0, "count": 3, "color": "#808080ff", "text": "#a", "on": true,
            "list": [1, 2], "only_left": 1})
    );
    assert_eq!(left.blend(&right, 1.0), right);
}

/// A 10 s clip, a text over output 2–6 s in box 0.1,0.1,0.2,0.1.
fn boxed() -> Project {
    let mut p = project(&[10.0]);
    let args = json!({"kind": "text", "at": 2, "duration": 4, "rect": "0.1,0.1,0.2,0.1"});
    run(&mut p, &add(args));
    p
}

#[test]
fn a_change_at_a_moment_becomes_a_motion_from_the_start() {
    let mut p = boxed();
    let answer = run(
        &mut p,
        &set(json!({"id": "a1", "at": 4, "x": 0.5, "rotation": 90})),
    );
    assert_eq!(answer, "аннотации ~1 a1");
    let a = annotation(&p, 0);
    let times: Vec<_> = a.keys.iter().map(|k| k.at).collect();
    assert_eq!(times, [ms(0), ms(2000)], "a key at the start brought along");
    assert_eq!(a.rect.x(), 0.1, "the annotation itself stays");
    let (x, w, turn) = drawn(&p, 0, 3.0);
    assert!(close(x, 0.3) && close(w, 0.2) && close(f64::from(turn), 45.0));
    let (x, _, _) = drawn(&p, 0, 5.5);
    assert!(close(x, 0.5), "the last key holds");
    let same = set(json!({"id": "a1", "at": 4, "x": 0.5}));
    assert_eq!(run(&mut p, &same), "", "the key holds it already");
    run(&mut p, &set(json!({"id": "a1", "w": 0.4})));
    let a = annotation(&p, 0);
    assert!(a.rect.w() == 0.4 && a.keys.iter().all(|k| k.rect.w() == 0.4));
    assert_eq!(
        a.keys[1].rect.x(),
        0.5,
        "without --at only the given fields"
    );
}

#[test]
fn key_add_keeps_the_look_and_keys_move_with_the_annotation() {
    let mut p = boxed();
    run(&mut p, &set(json!({"id": "a1", "at": 4, "x": 0.5})));
    let before = drawn(&p, 0, 3.0);
    run(&mut p, &key_add(json!({"id": "a1", "at": 3})));
    assert_eq!(annotation(&p, 0).keys.len(), 3);
    assert!(
        close(drawn(&p, 0, 3.0).0, before.0),
        "the key holds what was shown"
    );
    let again = key_add(json!({"id": "a1", "at": 3.0005}));
    assert_eq!(run(&mut p, &again), "", "within 1 ms");
    run(&mut p, &command::<AnnotMove>(json!({"id": "a1", "at": 5})));
    assert_eq!(annotation(&p, 0).range, range(5.0, 9.0));
    assert!(
        close(drawn(&p, 0, 6.0).0, before.0),
        "1 s after the start as before"
    );
}

#[test]
fn keys_stay_on_the_recording_through_a_cut_and_a_speed_up() {
    // Output 0.5–3.5 s is source 0.5–7 s: the cut 1–3 s inside, 4–8 s at 2×.
    let mut p = cut_and_fast();
    run(
        &mut p,
        &add(json!({"kind": "text", "at": 0.5, "duration": 3})),
    );
    run(&mut p, &set(json!({"id": "a1", "at": 3, "x": 0.65})));
    let a = annotation(&p, 0);
    assert_eq!(a.keys[1].at, ms(5500), "output 3 s is source 6 s");
    // Output 1.5 s is source 3.5 s: 3 s of 5.5 s between the keys.
    let s = 3.0 / 5.5;
    let x = 0.35 + 0.3 * s * s * (3.0 - 2.0 * s);
    assert!(close(drawn(&p, 0, 1.5).0, x));
    p.clips[0].speeds.clear();
    let map = TimeMap::build(&p);
    let (_, at) = map.locate(&a.media, crate::tests::secs(3.5)).unwrap();
    let t = at.as_secs_f64();
    assert!(
        close(drawn(&p, 0, t).0, x),
        "the same frame of the recording, the same place"
    );
}

#[test]
fn trimming_the_head_keeps_keys_on_their_frames() {
    let mut p = boxed();
    run(&mut p, &set(json!({"id": "a1", "at": 4, "x": 0.5})));
    run(&mut p, &set(json!({"id": "a1", "from": 3})));
    let a = annotation(&p, 0);
    let times: Vec<_> = a.keys.iter().map(|k| k.at).collect();
    assert_eq!(
        times,
        [ms(-1000), ms(1000)],
        "the start key is kept before the head"
    );
    assert!(
        close(drawn(&p, 0, 3.0).0, 0.3),
        "still the middle of the motion"
    );
}

#[test]
fn removing_the_last_key_keeps_its_look() {
    let mut p = boxed();
    run(&mut p, &set(json!({"id": "a1", "at": 4, "x": 0.5})));
    let error = fail(&mut p, &key_rm(json!({"id": "a1", "at": 3})));
    assert!(error.contains("нет ключа в 0:03.000"), "{error}");
    assert!(
        error.contains("ключи на готовом видео: 0:02.000, 0:04.000"),
        "{error}"
    );
    run(&mut p, &key_rm(json!({"id": "a1", "at": 2})));
    assert!(close(drawn(&p, 0, 2.0).0, 0.5), "one key holds all along");
    run(&mut p, &key_rm(json!({"id": "a1", "at": 4})));
    let a = annotation(&p, 0);
    assert!(a.keys.is_empty() && a.rect.x() == 0.5);
}

#[test]
fn copy_puts_a_key_elsewhere_or_into_an_annotation_of_the_same_kind() {
    let mut p = boxed();
    run(
        &mut p,
        &add(json!({"kind": "text", "at": 6, "duration": 2})),
    );
    run(
        &mut p,
        &add(json!({"kind": "shape", "at": 6, "duration": 2})),
    );
    run(&mut p, &set(json!({"id": "a1", "at": 4, "x": 0.5})));
    run(&mut p, &key_copy(json!({"id": "a1", "at": 4, "to": 5})));
    assert_eq!(annotation(&p, 0).keys[2].rect.x(), 0.5);
    let into = key_copy(json!({"id": "a1", "at": 2, "to": 7, "into": "a2"}));
    run(&mut p, &into);
    let b = annotation(&p, 1);
    assert_eq!((b.keys.len(), b.keys[0].at), (1, ms(1000)));
    assert_eq!(b.keys[0].rect, Rect::new(0.1, 0.1, 0.2, 0.1).unwrap());
    assert_eq!(run(&mut p, &into), "");
    let error = fail(
        &mut p,
        &key_copy(json!({"id": "a1", "at": 2, "to": 7, "into": "a3"})),
    );
    assert!(error.contains("одного вида"), "{error}");
    let error = fail(&mut p, &key_copy(json!({"id": "a1", "at": 3, "to": 5})));
    assert!(error.contains("нет ключа"), "{error}");
}

#[test]
fn key_times_must_be_inside_the_annotation() {
    let mut p = boxed();
    let error = fail(&mut p, &key_add(json!({"id": "a1", "at": 7})));
    assert!(error.contains("с 0:02.000 до 0:06.000"), "{error}");
    run(&mut p, &key_add(json!({"id": "a1", "at": 6})));
    assert_eq!(
        annotation(&p, 0).keys[0].at,
        ms(4000),
        "the end is a key time too"
    );
    let error = fail(&mut p, &set(json!({"id": "a1", "at": 3, "from": 2.5})));
    assert!(error.contains("без --at"), "{error}");
    let error = fail(&mut p, &key_add(json!({"id": "a9", "at": 3})));
    assert!(error.contains("a9"), "{error}");
}

#[test]
fn undo_redo_and_the_file_keep_keys() {
    let mut p = boxed();
    let start = p.clone();
    let mut history = History::new(100);
    let steps: [Box<dyn Command>; 4] = [
        Box::new(set(json!({"id": "a1", "at": 4, "x": 0.5}))),
        Box::new(key_add(json!({"id": "a1", "at": 3}))),
        Box::new(key_copy(json!({"id": "a1", "at": 4, "to": 5}))),
        Box::new(key_rm(json!({"id": "a1", "at": 2}))),
    ];
    for step in &steps {
        history.apply(&mut p, step.as_ref()).unwrap();
    }
    let last = p.clone();
    while history.undo(&mut p).is_some() {}
    assert_eq!(p, start);
    while history.redo(&mut p).is_some() {}
    assert_eq!(p, last);
    let text = serde_json::to_string(&p).unwrap();
    assert_eq!(serde_json::from_str::<Project>(&text).unwrap(), p);
    let stored = &p.effects["annotations"]["annotations"][0]["keys"][0];
    assert_eq!(
        stored["at"],
        json!(1_000_000),
        "µs of source time since the start"
    );
}
