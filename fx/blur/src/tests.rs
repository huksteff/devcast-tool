use dc_core::project::{Media, MediaKind};
use dc_core::{Clip, Command, History, PixelSize, Project, Range, Rect, SourceTime, TimeDelta};
use dc_fx_annotations::{AnnotAdd, AnnotAnim, Annotation, Annotations, keyed};
use dc_render::Rgba;
use serde_json::{Map, Value, json};

use crate::{BlurBody, BlurSet};

/// One 10 s recording with blur annotation a1 and text annotation a2, both 1–2 s.
pub(crate) fn project() -> Project {
    let mut p = Project::default();
    let id = p.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(10.0);
    p.clips.push(Clip {
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
    for kind in ["blur", "text"] {
        let add: AnnotAdd = serde_json::from_value(json!({"kind": kind, "at": 1})).unwrap();
        add.apply(&mut p).unwrap();
    }
    p
}

fn set(args: Value) -> BlurSet {
    serde_json::from_value(args).unwrap()
}

fn blur(p: &Project) -> Annotation {
    p.state::<Annotations>().unwrap().annotations[0].clone()
}

fn body(p: &Project) -> BlurBody {
    blur(p).body().unwrap()
}

/// The blur fields of `a` at key time `ms`, blended from the typed fields of its keys.
fn shown(a: &Annotation, ms: i64) -> BlurBody {
    let read = |body: &Map<String, Value>| -> BlurBody {
        serde_json::from_value(Value::Object(body.clone())).unwrap()
    };
    let keys: Vec<_> = a.keys.iter().map(|k| (k.at, read(&k.body))).collect();
    keyed(&read(&a.body), &keys, TimeDelta::from_millis(ms))
}

#[test]
fn a_new_blur_is_the_v1_one_without_a_tint() {
    let b = body(&project());
    assert_eq!(b, BlurBody::default());
    assert_eq!((b.strength, b.radius), (20.0, 8.0));
    assert_eq!(b.tint.0[3], 0, "pure blur");
}

#[test]
fn set_changes_only_the_given_fields() {
    let mut p = project();
    let args = json!({"id": "a1", "strength": 35, "tint": "#00000080"});
    assert_eq!(
        set(args).apply(&mut p).unwrap().to_string(),
        "аннотации ~1 a1"
    );
    let b = body(&p);
    assert_eq!((b.strength, b.tint), (35.0, Rgba([0, 0, 0, 128])));
    assert_eq!(b.radius, 8.0, "untouched");
    let same = set(json!({"id": "a1", "strength": 35}))
        .apply(&mut p)
        .unwrap();
    assert_eq!(same.to_string(), "", "nothing changed");
}

#[test]
fn set_rejects_bad_values_and_other_kinds() {
    let mut p = project();
    for (args, message) in [
        (json!({"id": "a1", "strength": 0.5}), "от 1 до 100"),
        (json!({"id": "a1", "strength": 101}), "от 1 до 100"),
        (json!({"id": "a1", "radius": -1}), "от 0 до 540"),
        (json!({"id": "a1", "radius": 541}), "от 0 до 540"),
        (json!({"id": "a2", "strength": 4}), "не размытие"),
        (json!({"id": "a9", "strength": 4}), "a9"),
    ] {
        let before = p.clone();
        let error = set(args.clone()).apply(&mut p).unwrap_err().to_string();
        assert!(error.contains(message), "{args}: {error}");
        assert_eq!(p, before, "{args}");
    }
    let bad = serde_json::from_value::<BlurSet>(json!({"id": "a1", "tint": "black"}));
    assert!(bad.is_err());
}

/// A change at 1.5 s keys the region; between the keys strength, radius and the tint (its
/// opacity too) blend with smoothstep.
#[test]
fn a_change_at_a_moment_keys_and_blends_every_field() {
    let mut p = project();
    let args = json!({"id": "a1", "at": 1.5, "strength": 60, "radius": 40,
        "tint": "#ff0000ff"});
    assert_eq!(
        set(args).apply(&mut p).unwrap().to_string(),
        "аннотации ~1 a1"
    );
    let a = blur(&p);
    let times: Vec<_> = a.keys.iter().map(|k| k.at.micros()).collect();
    assert_eq!(times, [0, 500_000], "a key at the start comes with it");
    assert_eq!(shown(&a, 0), BlurBody::default());
    let middle = shown(&a, 250);
    assert_eq!((middle.strength, middle.radius), (40.0, 24.0));
    assert_eq!(middle.tint, Rgba([128, 0, 0, 128]));
    assert_eq!(shown(&a, 900).strength, 60.0, "held after the last key");
    set(json!({"id": "a1", "radius": 0})).apply(&mut p).unwrap();
    let a = blur(&p);
    assert_eq!((shown(&a, 0).radius, shown(&a, 500).radius), (0.0, 0.0));
    assert_eq!(shown(&a, 500).strength, 60.0, "without --at: everywhere");
}

/// A blur takes fade, pop and slide; typewriter and draw-in are not for it.
#[test]
fn a_blur_enters_without_typing_or_drawing() {
    let mut p = project();
    let anim = |enter: &str| -> AnnotAnim {
        serde_json::from_value(json!({"id": "a1", "enter": enter})).unwrap()
    };
    for enter in ["fade", "pop", "slide_left"] {
        anim(enter).apply(&mut p).unwrap();
    }
    for enter in ["typewriter", "draw"] {
        assert!(anim(enter).apply(&mut p).is_err(), "{enter}");
    }
}

#[test]
fn undo_redo_and_the_project_file_keep_the_blur() {
    let mut p = project();
    let start = p.clone();
    let mut history = History::new(10);
    history
        .apply(&mut p, &set(json!({"id": "a1", "strength": 50})))
        .unwrap();
    history
        .apply(&mut p, &set(json!({"id": "a1", "tint": "#112233ff"})))
        .unwrap();
    let last = p.clone();
    while history.undo(&mut p).is_some() {}
    assert_eq!(p, start);
    while history.redo(&mut p).is_some() {}
    assert_eq!(p, last);
    let loaded: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(body(&loaded), body(&p));
    let stored = serde_json::to_value(blur(&loaded)).unwrap();
    assert_eq!(stored["body"]["tint"], json!("#112233ff"));
}

#[test]
fn ops_follow_the_rules() {
    let ops = crate::ops();
    assert_eq!(ops.len(), 1);
    let op = &ops[0];
    assert_eq!(op.name, "blur set");
    assert!(op.example.starts_with(op.name));
    let schema = serde_json::to_value((op.schema)()).unwrap();
    assert_eq!(schema["additionalProperties"], false);
}
