use dc_core::Project;
use serde_json::{Value, json};

use crate::tests::{add, command, cut_and_fast, fail, project, range, run, secs, state};
use crate::{AnnotLayer, AnnotMove};

fn layer(p: &mut Project, id: &str, step: &str) -> String {
    run(p, &command::<AnnotLayer>(json!({"id": id, "step": step})))
}

fn shown(p: &Project, t: f64) -> Vec<String> {
    let media = p.clips[0].media.path.clone();
    state(p)
        .shown_at(&media, secs(t))
        .iter()
        .map(|a| a.id.to_string())
        .collect()
}

/// a1, a3, a4 on track 0 over 0–4 s, a2 on track 1 over 0–4 s, a5 on track 0 over 6–7 s.
fn stack() -> Project {
    let mut p = project(&[10.0]);
    for (at, track) in [(0, 0), (0, 1), (0, 0), (0, 0), (6, 0)] {
        let duration = if at == 0 { 4 } else { 1 };
        let args = json!({"kind": "shape", "at": at, "duration": duration, "track": track});
        run(&mut p, &add(args));
    }
    p
}

#[test]
fn paint_order_is_by_track_then_by_order_within_it() {
    let p = stack();
    assert_eq!(shown(&p, 0.0), ["a1", "a3", "a4", "a2"]);
    assert_eq!(shown(&p, 3.999), ["a1", "a3", "a4", "a2"]);
    assert_eq!(shown(&p, 4.0), Vec::<String>::new(), "the end is exclusive");
    assert_eq!(shown(&p, 6.5), ["a5"]);
    assert_eq!(state(&p).shown_at("media/other.mp4", secs(1.0)).len(), 0);
}

#[test]
fn layer_moves_among_the_annotations_of_the_track() {
    let mut p = stack();
    assert_eq!(layer(&mut p, "a1", "up"), "аннотации ~1 a1");
    assert_eq!(shown(&p, 1.0), ["a3", "a1", "a4", "a2"]);
    assert_eq!(layer(&mut p, "a1", "top"), "аннотации ~1 a1");
    assert_eq!(shown(&p, 1.0), ["a3", "a4", "a1", "a2"]);
    assert_eq!(
        layer(&mut p, "a1", "up"),
        "",
        "already the last of its track"
    );
    assert_eq!(layer(&mut p, "a1", "top"), "");
    let ids: Vec<String> = state(&p).annot_ids().map(|i| i.to_string()).collect();
    assert_eq!(ids, ["a2", "a3", "a4", "a5", "a1"]);
    assert_eq!(layer(&mut p, "a1", "bottom"), "аннотации ~1 a1");
    assert_eq!(shown(&p, 1.0), ["a1", "a3", "a4", "a2"]);
    assert_eq!(layer(&mut p, "a1", "down"), "");
    layer(&mut p, "a4", "down");
    assert_eq!(shown(&p, 1.0), ["a1", "a4", "a3", "a2"]);
    assert_eq!(layer(&mut p, "a2", "top"), "", "alone on its track");
    let error = fail(
        &mut p,
        &command::<AnnotLayer>(json!({"id": "a9", "step": "up"})),
    );
    assert!(error.contains("a9 не найден"), "{error}");
}

fn move_to(args: Value) -> AnnotMove {
    command(args)
}

#[test]
fn move_keeps_the_output_length_and_lands_on_top_of_a_new_track() {
    let mut p = cut_and_fast();
    run(
        &mut p,
        &add(json!({"kind": "text", "at": 0, "duration": 0.5})),
    );
    run(
        &mut p,
        &add(json!({"kind": "text", "at": 0, "duration": 0.5})),
    );
    let changed = run(&mut p, &move_to(json!({"id": "a1", "at": 2.5})));
    assert_eq!(changed, "аннотации ~1 a1");
    assert_eq!(
        state(&p).annotations[0].range,
        range(5.0, 6.0),
        "0.5 s at 2×"
    );
    run(&mut p, &move_to(json!({"id": "a1", "at": 0.75})));
    assert_eq!(
        state(&p).annotations[0].range,
        range(0.75, 3.25),
        "over the cut 1–3 s"
    );
    run(&mut p, &move_to(json!({"id": "a1", "at": 6})));
    let a = &state(&p).annotations[0];
    assert_eq!(
        (a.media.as_str(), a.range),
        (p.clips[1].media.path.as_str(), range(0.0, 0.5))
    );
    run(&mut p, &move_to(json!({"id": "a1", "at": 0})));
    assert_eq!(shown(&p, 0.1), ["a1", "a2"]);
    run(&mut p, &move_to(json!({"id": "a1", "track": 1})));
    run(&mut p, &move_to(json!({"id": "a2", "track": 1})));
    assert_eq!(
        shown(&p, 0.1),
        ["a1", "a2"],
        "a2 arrived later, so it lies above"
    );
    assert_eq!(state(&p).annotations[1].track, 1);
    assert_eq!(run(&mut p, &move_to(json!({"id": "a2", "track": 1}))), "");
}

#[test]
fn move_explains_what_is_wrong() {
    let mut p = cut_and_fast();
    run(
        &mut p,
        &add(json!({"kind": "text", "at": 0, "duration": 0.5})),
    );
    for (args, message) in [
        (json!({"id": "a1"}), "куда двигать"),
        (json!({"id": "a1", "at": 5.8}), "стык клипов"),
        (json!({"id": "a1", "at": 15.8}), "позже конца видео"),
        (json!({"id": "a1", "track": 40}), "от 0 до 31"),
    ] {
        let error = fail(&mut p, &move_to(args.clone()));
        assert!(error.contains(message), "{args}: {error}");
    }
    p.clips[0].cuts.insert(0, range(0.0, 0.9));
    let error = fail(&mut p, &move_to(json!({"id": "a1", "at": 2})));
    assert!(error.contains("не видна в готовом видео"), "{error}");
}
