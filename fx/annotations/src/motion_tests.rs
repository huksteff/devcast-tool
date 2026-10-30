use dc_core::{OutputTime, Project, TimeDelta, TimeMap};
use serde_json::{Value, json};

use crate::tests::{add, command, cut_and_fast, fail, project, run, state};
use crate::{Animation, AnnotAnim, Placement, Pose, STAGE_DEFAULT};
use dc_core::PixelSize;

fn anim(args: Value) -> AnnotAnim {
    command(args)
}

/// The pose of annotation `index` at output second `t`.
fn pose(p: &Project, index: usize, t: f64) -> Pose {
    let map = TimeMap::build(p);
    let t = OutputTime::from_secs_f64(t);
    let segment = map.segment_at(t).unwrap();
    Pose::at(&state(p).annotations[index], &map, segment.source_at(t), t)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// A 10 s clip with one annotation over output 2–4 s, animated as `args` say.
fn animated(args: Value) -> Project {
    let mut p = project(&[10.0]);
    run(
        &mut p,
        &add(json!({"kind": "text", "at": 2, "duration": 2})),
    );
    let mut full = json!({"id": "a1"});
    if let (Some(into), Value::Object(from)) = (full.as_object_mut(), args) {
        into.extend(from);
    }
    run(&mut p, &anim(full));
    p
}

#[test]
fn fade_is_linear_and_the_exit_runs_backwards() {
    let p = animated(json!({"enter": "fade", "enter_duration": 0.4, "exit": "fade"}));
    let opacity = |t| pose(&p, 0, t).opacity;
    assert!(close(opacity(2.0), 0.0), "the first frame is not in yet");
    assert!(close(opacity(2.1), 0.25));
    assert!(close(opacity(2.3), 0.75));
    assert_eq!(pose(&p, 0, 3.0), Pose::STILL);
    assert!(close(opacity(3.85), 0.5), "0.3 s exit by default");
}

#[test]
fn pop_grows_with_a_slight_overshoot() {
    let p = animated(json!({"enter": "pop", "enter_duration": 1}));
    let at = |t| pose(&p, 0, t);
    assert!(close(at(2.0).scale, 0.58) && close(at(2.0).opacity, 0.0));
    // smoothstep(0.5) = 0.5: 0.58 + 0.21 + 0.14
    assert!(close(at(2.5).scale, 0.93) && close(at(2.5).opacity, 0.5));
    let late = at(2.0 + 0.8);
    assert!(late.scale > 1.0 && late.scale < 1.002, "{}", late.scale);
    assert_eq!(at(3.0), Pose::STILL);
}

#[test]
fn slides_come_from_their_side_and_leave_to_it() {
    let shift = |name: &str, t: f64| {
        let p = animated(json!({"enter": name, "enter_duration": 1, "exit": name,
            "exit_duration": 1}));
        pose(&p, 0, t).shift
    };
    assert_eq!(shift("slide_left", 2.0), [-96.0, 0.0]);
    assert_eq!(shift("slide_right", 2.0), [96.0, 0.0]);
    assert_eq!(shift("slide_top", 2.0), [0.0, -72.0]);
    assert_eq!(shift("slide_bottom", 2.0), [0.0, 72.0]);
    assert_eq!(shift("slide_left", 2.5), [-48.0, 0.0]);
    assert_eq!(
        shift("slide_left", 3.5),
        [-48.0, 0.0],
        "the exit goes back left"
    );
    let p = animated(json!({"enter": "slide_bottom", "enter_duration": 1}));
    assert!(close(pose(&p, 0, 2.5).opacity, 0.5));
}

#[test]
fn typewriter_reveals_linearly_and_erases_from_the_end() {
    let p = animated(
        json!({"enter": "typewriter", "enter_duration": 1, "exit": "typewriter",
        "exit_duration": 0.5}),
    );
    let reveal = |t| pose(&p, 0, t).reveal;
    assert!(close(reveal(2.25), 0.25));
    assert!(close(reveal(3.75), 0.5));
    assert!(close(pose(&p, 0, 2.25).opacity, 1.0));
}

#[test]
fn stages_run_on_output_time_over_a_speed_up_and_a_cut() {
    // Output 2–4 s shows source 4–8 s at 2×: the fade still takes 0.4 s of output.
    let mut p = cut_and_fast();
    run(
        &mut p,
        &add(json!({"kind": "text", "at": 2, "duration": 2})),
    );
    // Output 0.5–1.5 s shows source 0.5–1 s, the cut 1–3 s, then 3–3.5 s.
    run(
        &mut p,
        &add(json!({"kind": "text", "at": 0.5, "duration": 1})),
    );
    for id in ["a1", "a2"] {
        run(
            &mut p,
            &anim(json!({"id": id, "enter": "fade", "enter_duration": 0.8,
            "exit": "fade", "exit_duration": 0.2})),
        );
    }
    let opacity = |index, t| pose(&p, index, t).opacity;
    assert!(close(opacity(0, 2.2), 0.25));
    assert!(close(opacity(0, 3.9), 0.5));
    assert!(
        close(opacity(1, 1.1), 0.75),
        "the cut does not skip the entrance"
    );
    assert!(close(opacity(1, 1.45), 0.25));
}

#[test]
fn stages_fit_into_the_shown_length_the_entrance_first() {
    let p = animated(json!({"enter": "fade", "enter_duration": 10, "exit": "fade"}));
    assert!(
        close(pose(&p, 0, 3.0).opacity, 0.5),
        "the entrance takes all 2 s"
    );
    let p = animated(
        json!({"enter": "fade", "enter_duration": 1.5, "exit": "fade",
        "exit_duration": 1}),
    );
    assert!(
        close(pose(&p, 0, 3.75).opacity, 0.5),
        "0.5 s left for the exit"
    );
    let p = animated(json!({"exit": "fade", "exit_duration": 10}));
    assert!(
        close(pose(&p, 0, 3.0).opacity, 0.5),
        "no entrance: the exit takes it all"
    );
}

#[test]
fn placement_scales_around_the_centre_and_shifts_in_reference_px() {
    let p = animated(json!({"enter": "pop", "enter_duration": 1}));
    let a = &state(&p).annotations[0];
    let canvas = PixelSize {
        width: 960,
        height: 540,
    };
    let still = Placement::of(a, canvas);
    let pose = Pose {
        opacity: 0.5,
        shift: [-96.0, 72.0],
        scale: 0.5,
        reveal: 0.25,
    };
    let moved = still.posed(&pose);
    let centre = |p: &Placement| [p.rect.x + p.rect.w / 2.0, p.rect.y + p.rect.h / 2.0];
    let [cx, cy] = centre(&still);
    assert_eq!(centre(&moved), [cx - 48.0, cy + 36.0], "k = 0.5 at 540 px");
    assert_eq!(
        [moved.rect.w, moved.rect.h],
        [still.rect.w / 2.0, still.rect.h / 2.0]
    );
    assert_eq!((moved.k, moved.opacity, moved.reveal), (0.25, 0.5, 0.25));
    assert_eq!(still.posed(&Pose::STILL), still);
}

#[test]
fn anim_sets_stages_and_keeps_the_rest() {
    let mut p = animated(json!({"enter": "pop"}));
    let a = &state(&p).annotations[0];
    assert_eq!(a.enter.animation, Animation::Pop);
    assert_eq!(a.enter.duration, STAGE_DEFAULT);
    assert_eq!(a.exit.animation, Animation::None);
    let changed = run(&mut p, &anim(json!({"id": "a1", "enter_duration": 0.5})));
    assert_eq!(changed, "аннотации ~1 a1");
    let a = &state(&p).annotations[0];
    assert_eq!(a.enter.animation, Animation::Pop, "the animation stays");
    assert_eq!(a.enter.duration, TimeDelta::from_millis(500));
    assert_eq!(run(&mut p, &anim(json!({"id": "a1", "enter": "pop"}))), "");
}

#[test]
fn anim_rejects_what_does_not_fit() {
    let mut p = project(&[10.0]);
    run(&mut p, &add(json!({"kind": "shape", "at": 1})));
    run(&mut p, &add(json!({"kind": "text", "at": 1})));
    run(&mut p, &add(json!({"kind": "drawing", "at": 1})));
    let error = fail(&mut p, &anim(json!({"id": "a1", "enter": "typewriter"})));
    assert!(error.contains("typewriter есть только у текста"), "{error}");
    let error = fail(&mut p, &anim(json!({"id": "a2", "exit": "draw"})));
    assert!(error.contains("вида text"), "{error}");
    for seconds in [-0.1, 10.5] {
        let error = fail(&mut p, &anim(json!({"id": "a2", "exit_duration": seconds})));
        assert!(error.contains("от 0 до 10 с"), "{error}");
    }
    let error = fail(&mut p, &anim(json!({"id": "a9", "enter": "fade"})));
    assert!(error.contains("a9"), "{error}");
    run(&mut p, &anim(json!({"id": "a1", "enter": "draw"})));
    run(
        &mut p,
        &anim(json!({"id": "a3", "exit": "draw", "exit_duration": 0})),
    );
    run(&mut p, &anim(json!({"id": "a2", "enter": "typewriter"})));
}

#[test]
fn stages_are_stored_only_when_set() {
    let mut p = project(&[10.0]);
    run(&mut p, &add(json!({"kind": "text", "at": 1})));
    let stored = serde_json::to_value(&state(&p).annotations[0]).unwrap();
    assert!(stored.get("enter").is_none() && stored.get("exit").is_none());
    run(
        &mut p,
        &anim(json!({"id": "a1", "exit": "slide_top", "exit_duration": 1.25})),
    );
    let stored = serde_json::to_value(&state(&p).annotations[0]).unwrap();
    assert_eq!(
        stored["exit"],
        json!({"animation": "slide_top", "duration": 1_250_000})
    );
    let back: crate::Annotation = serde_json::from_value(stored).unwrap();
    assert_eq!(back, state(&p).annotations[0]);
}
