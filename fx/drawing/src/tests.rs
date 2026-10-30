use dc_core::project::{Media, MediaKind};
use dc_core::{Command, History, PixelSize, Project, Range, Rect, SourceTime, TimeDelta};
use dc_fx_annotations::{AnnotAdd, Annotation, Annotations, keyed};
use dc_render::Rgba;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::{
    DrawingAdd, DrawingBody, DrawingClear, DrawingRm, DrawingSet, ERASER_WIDTH, PENCIL_WIDTH,
    Stroke, StrokeTool,
};

/// One 10 s recording with drawing annotation a1 and shape annotation a2, both output 1–2 s.
fn project() -> Project {
    let mut p = Project::default();
    let id = p.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(10.0);
    p.clips.push(dc_core::Clip {
        id,
        media: Media {
            path: "media/запись 1.mp4".into(),
            kind: MediaKind::Recording,
            duration,
            size: PixelSize {
                width: 1920,
                height: 1080,
            },
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration).unwrap(),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
        audio: Default::default(),
    });
    for kind in ["drawing", "shape"] {
        run::<AnnotAdd>(&mut p, json!({"kind": kind, "at": 1}));
    }
    p
}

fn command<C: DeserializeOwned>(args: Value) -> C {
    serde_json::from_value(args).unwrap()
}

/// The diff `C` from `args` makes, as text.
fn run<C: Command + DeserializeOwned>(p: &mut Project, args: Value) -> String {
    command::<C>(args).apply(p).unwrap().to_string()
}

fn fails<C: Command + DeserializeOwned>(p: &mut Project, args: Value) -> String {
    let before = p.clone();
    let error = command::<C>(args.clone()).apply(p).unwrap_err().to_string();
    assert_eq!(*p, before, "{args}");
    error
}

fn drawing(p: &Project) -> Annotation {
    p.state::<Annotations>().unwrap().annotations[0].clone()
}

fn body(p: &Project) -> DrawingBody {
    drawing(p).body().unwrap()
}

/// Pencil zigzag, eraser across it, red straight line.
fn three_strokes() -> Project {
    let mut p = project();
    run::<DrawingAdd>(&mut p, json!({"id": "a1", "points": "0,1,0.5,0,1,1"}));
    run::<DrawingAdd>(
        &mut p,
        json!({"id": "a1", "points": "0,0.5,1,0.5", "tool": "eraser"}),
    );
    run::<DrawingAdd>(
        &mut p,
        json!({"id": "a1", "points": "0.1, 0.9, 0.9 ,0.1", "color": "#ff0000ff", "width": 10}),
    );
    p
}

#[test]
fn a_new_drawing_is_empty_and_opaque() {
    let b = body(&project());
    assert_eq!(b, DrawingBody::default());
    assert!(b.strokes.is_empty());
    assert_eq!(b.opacity, 1.0);
}

#[test]
fn strokes_are_added_in_order_with_tool_defaults() {
    let p = three_strokes();
    let strokes = body(&p).strokes;
    assert_eq!(
        strokes[0],
        Stroke {
            tool: StrokeTool::Pencil,
            color: Rgba([0x25, 0x63, 0xeb, 255]),
            width: PENCIL_WIDTH,
            points: vec![[0.0, 1.0], [0.5, 0.0], [1.0, 1.0]],
        },
        "v1 blue, 4 px"
    );
    assert_eq!(
        (strokes[1].tool, strokes[1].width),
        (StrokeTool::Eraser, ERASER_WIDTH)
    );
    assert_eq!(
        strokes[2].points,
        [[0.1, 0.9], [0.9, 0.1]],
        "two points: a line"
    );
    assert_eq!(
        (strokes[2].color, strokes[2].width),
        (Rgba([255, 0, 0, 255]), 10.0)
    );
    let mut dot = project();
    run::<DrawingAdd>(&mut dot, json!({"id": "a1", "points": "0.5,0.5"}));
    assert_eq!(
        body(&dot).strokes[0].points,
        [[0.5, 0.5]],
        "one point: a dot"
    );
}

#[test]
fn bad_strokes_and_other_kinds_are_refused() {
    let mut p = three_strokes();
    for (args, message) in [
        (json!({"id": "a1", "points": "0.5"}), "пары x,y"),
        (json!({"id": "a1", "points": "0,0,1"}), "пары x,y"),
        (json!({"id": "a1", "points": ""}), "пары x,y"),
        (json!({"id": "a1", "points": "0,0;1,1"}), "пары x,y"),
        (json!({"id": "a1", "points": "0,0,1.5,1"}), "от 0 до 1"),
        (
            json!({"id": "a1", "points": "0,0", "width": 0.5}),
            "от 1 до 64",
        ),
        (
            json!({"id": "a1", "points": "0,0", "width": 65}),
            "от 1 до 64",
        ),
        (
            json!({"id": "a1", "points": "0,0", "tool": "eraser", "color": "#ff0000ff"}),
            "у ластика нет цвета",
        ),
        (json!({"id": "a2", "points": "0,0"}), "не рисунок"),
        (json!({"id": "a9", "points": "0,0"}), "a9"),
    ] {
        let error = fails::<DrawingAdd>(&mut p, args.clone());
        assert!(error.contains(message), "{args}: {error}");
    }
    for (args, message) in [
        (json!({"id": "a1", "stroke": 4, "width": 2}), "от 1 до 3"),
        (json!({"id": "a1", "stroke": 0, "width": 2}), "от 1 до 3"),
        (
            json!({"id": "a1", "stroke": 2, "color": "#ff0000ff"}),
            "у ластика нет цвета",
        ),
        (json!({"id": "a1", "opacity": 1.5}), "от 0 до 1"),
        (json!({"id": "a2", "opacity": 0.5}), "не рисунок"),
    ] {
        let error = fails::<DrawingSet>(&mut p, args.clone());
        assert!(error.contains(message), "{args}: {error}");
    }
    let error = fails::<DrawingRm>(&mut project(), json!({"id": "a1", "stroke": 1}));
    assert!(error.contains("рисунок пуст"), "{error}");
    let unknown =
        serde_json::from_value::<DrawingAdd>(json!({"id": "a1", "points": "0,0", "tool": "line"}));
    assert!(unknown.is_err(), "a line is a pencil stroke of two points");
}

#[test]
fn set_changes_one_stroke_or_every_pencil_stroke() {
    let mut p = three_strokes();
    let green = Rgba([0x22, 0xc5, 0x5e, 255]);
    let diff = run::<DrawingSet>(&mut p, json!({"id": "a1", "stroke": 2, "width": 30}));
    assert_eq!(diff, "аннотации ~1 a1");
    assert_eq!(body(&p).strokes[1].width, 30.0, "the eraser grows");
    run::<DrawingSet>(
        &mut p,
        json!({"id": "a1", "color": "#22c55eff", "width": 2, "opacity": 0.5}),
    );
    let b = body(&p);
    assert_eq!(b.opacity, 0.5);
    assert_eq!([b.strokes[0].color, b.strokes[2].color], [green, green]);
    assert_eq!([b.strokes[0].width, b.strokes[2].width], [2.0, 2.0]);
    assert_eq!(b.strokes[1].width, 30.0, "the eraser keeps its size");
    let same = run::<DrawingSet>(&mut p, json!({"id": "a1", "opacity": 0.5}));
    assert_eq!(same, "", "nothing changed");
}

#[test]
fn rm_takes_one_stroke_and_clear_takes_all() {
    let mut p = three_strokes();
    assert_eq!(
        run::<DrawingRm>(&mut p, json!({"id": "a1", "stroke": 2})),
        "аннотации ~1 a1"
    );
    let tools: Vec<_> = body(&p).strokes.iter().map(|s| s.tool).collect();
    assert_eq!(tools, [StrokeTool::Pencil, StrokeTool::Pencil]);
    assert_eq!(body(&p).strokes[1].width, 10.0, "the line is number 2 now");
    run::<DrawingClear>(&mut p, json!({"id": "a1"}));
    assert!(body(&p).strokes.is_empty());
    assert_eq!(run::<DrawingClear>(&mut p, json!({"id": "a1"})), "");
}

/// The drawing fields of `a` at key time `ms`.
fn shown(a: &Annotation, ms: i64) -> DrawingBody {
    let read = |body: &Map<String, Value>| -> DrawingBody {
        serde_json::from_value(Value::Object(body.clone())).unwrap()
    };
    let keys: Vec<_> = a.keys.iter().map(|k| (k.at, read(&k.body))).collect();
    keyed(&read(&a.body), &keys, TimeDelta::from_millis(ms))
}

#[test]
fn a_stroke_added_at_a_moment_appears_at_its_key_and_opacity_blends() {
    let mut p = three_strokes();
    run::<DrawingAdd>(&mut p, json!({"id": "a1", "points": "0.5,0.5", "at": 1.5}));
    run::<DrawingSet>(&mut p, json!({"id": "a1", "opacity": 0.2, "at": 1.5}));
    let a = drawing(&p);
    let times: Vec<_> = a.keys.iter().map(|k| k.at.micros()).collect();
    assert_eq!(times, [0, 500_000], "the start is kept in a key");
    assert_eq!(
        body(&p).strokes.len(),
        3,
        "the annotation itself is as it was"
    );
    let mid = shown(&a, 250);
    assert_eq!(mid.strokes.len(), 3, "strokes step at the key");
    assert!((mid.opacity - 0.6).abs() < 1e-9, "{}", mid.opacity);
    assert_eq!(shown(&a, 500).strokes.len(), 4);
    run::<DrawingSet>(&mut p, json!({"id": "a1", "color": "#000000ff"}));
    let black = Rgba([0, 0, 0, 255]);
    let a = drawing(&p);
    assert!(
        a.keys.iter().all(|k| {
            let b: DrawingBody = serde_json::from_value(Value::Object(k.body.clone())).unwrap();
            b.strokes
                .iter()
                .filter(|s| s.tool == StrokeTool::Pencil)
                .all(|s| s.color == black)
        }),
        "without --at in every key"
    );
}

#[test]
fn undo_redo_and_the_project_file_keep_the_drawing() {
    let mut p = project();
    let start = p.clone();
    let mut history = History::new(10);
    let add: DrawingAdd = command(json!({"id": "a1", "points": "0,0,1,1"}));
    history.apply(&mut p, &add).unwrap();
    let set: DrawingSet = command(json!({"id": "a1", "opacity": 0.4}));
    history.apply(&mut p, &set).unwrap();
    let last = p.clone();
    while history.undo(&mut p).is_some() {}
    assert_eq!(p, start);
    while history.redo(&mut p).is_some() {}
    assert_eq!(p, last);
    let loaded: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(body(&loaded), body(&last));
    let stored = serde_json::to_value(drawing(&loaded)).unwrap();
    assert_eq!(
        stored["body"]["strokes"],
        json!([{"tool": "pencil", "color": "#2563ebff", "width": 4.0, "points": [[0.0, 0.0], [1.0, 1.0]]}])
    );
}

#[test]
fn ops_follow_the_rules() {
    let names: Vec<_> = crate::ops().iter().map(|op| op.name).collect();
    assert_eq!(
        names,
        ["drawing add", "drawing set", "drawing rm", "drawing clear"]
    );
    for op in crate::ops() {
        assert!(op.example.starts_with(op.name));
        let schema = serde_json::to_value((op.schema)()).unwrap();
        assert_eq!(schema["additionalProperties"], false);
    }
}
