use dc_core::project::{Media, MediaKind, SpeedRange};
use dc_core::{Clip, Command, History, PixelSize, Project, Range, Rect, SourceTime, TimeDelta};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{AnnotAdd, AnnotKind, AnnotRm, AnnotSet, Annotations, AnnotationsState, Kind};

pub(crate) fn secs(s: f64) -> SourceTime {
    SourceTime::from_secs_f64(s)
}

pub(crate) fn range(from: f64, to: f64) -> Range<dc_core::time::Source> {
    Range::new(secs(from), secs(to)).unwrap()
}

pub(crate) fn project(lengths: &[f64]) -> Project {
    let mut p = Project::default();
    for length in lengths {
        let id = p.clip_ids.allocate();
        let duration = TimeDelta::from_secs_f64(*length);
        p.clips.push(Clip {
            id,
            media: Media {
                path: format!("media/{id}.mp4"),
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
    }
    p
}

/// Clip of 10 s with a cut 1–3 s and 2× on 4–8 s (output 0–6 s), then a 10 s clip (6–16 s).
pub(crate) fn cut_and_fast() -> Project {
    let mut p = project(&[10.0, 10.0]);
    let c = &mut p.clips[0];
    c.cuts.push(range(1.0, 3.0));
    c.speeds.push(SpeedRange {
        range: range(4.0, 8.0),
        factor: 2.0,
    });
    p
}

pub(crate) fn command<T: for<'de> Deserialize<'de>>(args: Value) -> T {
    serde_json::from_value(args).unwrap()
}

pub(crate) fn add(args: Value) -> AnnotAdd {
    command(args)
}

pub(crate) fn state(p: &Project) -> AnnotationsState {
    p.state::<Annotations>().unwrap()
}

pub(crate) fn run(p: &mut Project, command: &dyn Command) -> String {
    command.apply(p).unwrap().to_string()
}

pub(crate) fn fail(p: &mut Project, command: &dyn Command) -> String {
    let before = p.clone();
    let error = command.apply(p).unwrap_err().to_string();
    assert_eq!(*p, before, "failed command changed the project");
    error
}

#[test]
fn add_stores_source_time_box_and_defaults() {
    let mut p = project(&[10.0]);
    assert_eq!(
        run(&mut p, &add(json!({"kind": "text", "at": 2}))),
        "аннотации +1 a1"
    );
    let a = &state(&p).annotations[0];
    assert_eq!((a.kind, &a.media), (Kind::Text, &p.clips[0].media.path));
    assert_eq!((a.range, a.track, a.rotation), (range(2.0, 3.0), 0, 0.0));
    assert_eq!(a.rect, Rect::new(0.35, 0.4, 0.3, 0.2).unwrap());
    let args = json!({"kind": "blur", "at": 1, "duration": 0.5, "track": 3,
        "rect": "-0.1,0.9,0.5,0.4", "rotation": -15});
    run(&mut p, &add(args));
    let b = &state(&p).annotations[1];
    assert_eq!((b.kind, b.range, b.track), (Kind::Blur, range(1.0, 1.5), 3));
    assert_eq!(
        (b.rect, b.rotation),
        (Rect::new(-0.1, 0.9, 0.5, 0.4).unwrap(), -15.0)
    );
}

#[test]
fn add_maps_output_time_through_a_cut_a_speed_up_and_clips() {
    let mut p = cut_and_fast();
    run(&mut p, &add(json!({"kind": "text", "at": 1.5})));
    run(&mut p, &add(json!({"kind": "shape", "at": 0.5})));
    run(&mut p, &add(json!({"kind": "image", "at": 5})));
    run(
        &mut p,
        &add(json!({"kind": "drawing", "at": 6.5, "duration": 2})),
    );
    let a = state(&p).annotations;
    assert_eq!(
        a[0].range,
        range(3.5, 5.0),
        "1 s of output, half of it at 2×"
    );
    assert_eq!(a[1].range, range(0.5, 3.5), "the cut 1–3 s is inside");
    assert_eq!(a[2].range, range(9.0, 10.0), "ends at the end of the clip");
    assert_eq!(
        (a[3].media.as_str(), a[3].range),
        (p.clips[1].media.path.as_str(), range(0.5, 2.5))
    );
}

#[test]
fn add_rejects_bad_arguments() {
    let mut p = cut_and_fast();
    for (args, message) in [
        (json!({"at": 5.5}), "стык клипов разных записей в 0:06"),
        (json!({"at": 15.5}), "позже конца видео"),
        (json!({"at": 16}), "не раньше конца видео"),
        (json!({"at": -1}), "раньше начала"),
        (json!({"at": 1, "duration": 0}), "больше нуля"),
        (json!({"at": 1, "duration": 0.05}), "короче 0.1 с"),
        (json!({"at": 1, "track": 32}), "от 0 до 31"),
        (json!({"at": 1, "rect": "0,0,0.001,0.5"}), "от 0.01 до 10"),
        (json!({"at": 1, "rect": "11,0,0.5,0.5"}), "слишком далеко"),
        (json!({"at": 1, "rotation": 361}), "от -360 до 360"),
    ] {
        let mut args = args;
        args["kind"] = json!("text");
        let error = fail(&mut p, &add(args.clone()));
        assert!(error.contains(message), "{args}: {error}");
    }
    let wrong_kind = serde_json::from_value::<AnnotAdd>(json!({"kind": "click", "at": 1}));
    assert!(wrong_kind.is_err());
    let error = fail(&mut project(&[]), &add(json!({"kind": "text", "at": 0})));
    assert!(error.contains("нет клипов"), "{error}");
}

#[test]
fn set_changes_time_box_and_rotation() {
    let mut p = cut_and_fast();
    run(&mut p, &add(json!({"kind": "text", "at": 0})));
    let set = |args: Value| command::<AnnotSet>(args);
    let changed = set(json!({"id": "a1", "to": 2.5, "x": 0.1, "h": 0.5, "rotation": 30}));
    assert_eq!(run(&mut p, &changed), "аннотации ~1 a1");
    let a = &state(&p).annotations[0];
    assert_eq!(a.range, range(0.0, 5.0));
    assert_eq!(a.rect, Rect::new(0.1, 0.4, 0.3, 0.5).unwrap());
    assert_eq!(a.rotation, 30.0);
    run(
        &mut p,
        &set(json!({"id": "a1", "from": 1, "rect": "0,0,1,1"})),
    );
    let a = &state(&p).annotations[0];
    assert_eq!((a.range, a.rect), (range(3.0, 5.0), Rect::FULL));
    assert_eq!(run(&mut p, &set(json!({"id": "a1", "rotation": 30}))), "");
    for (args, message) in [
        (
            json!({"id": "a1", "rect": "0,0,1,1", "w": 0.5}),
            "не вместе",
        ),
        (json!({"id": "a1", "from": 2.5}), "позже начала"),
        (json!({"id": "a1", "to": 7}), "стык клипов"),
        (json!({"id": "a1", "to": 1.05}), "короче 0.1 с"),
        (json!({"id": "a1", "w": 20}), "от 0.01 до 10"),
        (json!({"id": "a1", "rotation": -400}), "от -360"),
        (json!({"id": "a7", "x": 0}), "a7 не найден (есть a1)"),
    ] {
        let error = fail(&mut p, &set(args.clone()));
        assert!(error.contains(message), "{args}: {error}");
    }
}

#[test]
fn rm_removes_and_ids_are_not_reused() {
    let mut p = project(&[10.0]);
    run(&mut p, &add(json!({"kind": "text", "at": 1})));
    run(&mut p, &add(json!({"kind": "text", "at": 2})));
    let rm = |id: &str| command::<AnnotRm>(json!({ "id": id }));
    assert_eq!(run(&mut p, &rm("a2")), "аннотации -1 a2");
    run(&mut p, &add(json!({"kind": "shape", "at": 3})));
    let ids: Vec<String> = state(&p).annot_ids().map(|i| i.to_string()).collect();
    assert_eq!(ids, ["a1", "a3"]);
    let error = fail(&mut p, &rm("a2"));
    assert!(error.contains("не найден"), "{error}");
}

#[test]
fn undo_redo_returns_the_same_state() {
    let mut p = project(&[10.0]);
    let mut history = History::new(100);
    let steps: [Box<dyn Command>; 5] = [
        Box::new(add(json!({"kind": "text", "at": 1}))),
        Box::new(command::<AnnotSet>(json!({"id": "a1", "rotation": 45}))),
        Box::new(command::<crate::AnnotMove>(
            json!({"id": "a1", "at": 4, "track": 2}),
        )),
        Box::new(command::<crate::AnnotLayer>(
            json!({"id": "a1", "step": "top"}),
        )),
        Box::new(command::<AnnotRm>(json!({"id": "a1"}))),
    ];
    history.apply(&mut p, steps[0].as_ref()).unwrap();
    let after_add = p.clone();
    for step in &steps[1..] {
        history.apply(&mut p, step.as_ref()).unwrap();
    }
    let last = p.clone();
    while history.undo(&mut p).is_some() {}
    assert_eq!(p, project(&[10.0]));
    history.redo(&mut p);
    assert_eq!(p, after_add);
    while history.redo(&mut p).is_some() {}
    assert_eq!(p, last);
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Body {
    content: String,
    size: f64,
}

#[test]
fn kind_fields_round_trip_and_are_listed() {
    let mut p = project(&[10.0]);
    run(
        &mut p,
        &add(json!({"kind": "text", "at": 1, "rotation": 15})),
    );
    let mut s = state(&p);
    assert_eq!(s.annotations[0].body::<Body>().unwrap(), Body::default());
    let body = Body {
        content: "Кириллица 🎬".into(),
        size: 32.0,
    };
    s.annotations[0].set_body(&body).unwrap();
    assert!(s.annotations[0].set_body(&3).is_err());
    p.set_state::<Annotations>(&s).unwrap();
    let json = serde_json::to_string(&p).unwrap();
    let loaded: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(state(&loaded), s);
    assert_eq!(state(&loaded).annotations[0].body::<Body>().unwrap(), body);
    let items = (crate::effect().items)(&p).unwrap();
    assert_eq!((items[0].id.as_str(), items[0].list), ("a1", "annots"));
    let dc_core::Place::Source { range: span, .. } = &items[0].place else {
        panic!("{:?}", items[0].place)
    };
    assert_eq!(*span, range(1.0, 2.0));
    assert_eq!(items[0].fields["kind"], "text");
    assert_eq!(items[0].fields["rotation"], 15.0);
    assert_eq!(items[0].fields["rect"]["w"], 0.3);
    assert_eq!(items[0].fields["body"]["content"], "Кириллица 🎬");
}

#[test]
fn ops_follow_the_rules() {
    let ops = crate::ops();
    let names: Vec<&str> = ops.iter().map(|op| op.name).collect();
    assert_eq!(
        names,
        [
            "annot add",
            "annot set",
            "annot anim",
            "annot key add",
            "annot key rm",
            "annot key copy",
            "annot move",
            "annot layer",
            "annot rm"
        ]
    );
    for op in &ops {
        assert!(op.example.starts_with(op.name), "{}", op.example);
        let schema = serde_json::to_value((op.schema)()).unwrap();
        assert_eq!(schema["additionalProperties"], false, "{}", op.name);
    }
    assert_eq!(
        crate::effect().id_prefixes,
        [<AnnotKind as dc_core::IdKind>::PREFIX]
    );
}
