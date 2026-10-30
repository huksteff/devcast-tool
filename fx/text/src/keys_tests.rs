use dc_core::{Command, Project, TimeDelta};
use dc_fx_annotations::{AnnotKeyAdd, Annotation, Annotations, keyed};
use dc_render::Rgba;
use serde_json::{Value, json};

use crate::TextBody;
use crate::tests::{project, set};

fn run(p: &mut Project, command: &dyn Command) -> String {
    command.apply(p).unwrap().to_string()
}

fn text(p: &Project) -> Annotation {
    p.state::<Annotations>().unwrap().annotations[0].clone()
}

/// The text fields of `a` at key time `ms`, blended from the typed fields of its keys.
fn shown(a: &Annotation, ms: i64) -> TextBody {
    let read = |body: &serde_json::Map<String, Value>| -> TextBody {
        serde_json::from_value(Value::Object(body.clone())).unwrap()
    };
    let keys: Vec<_> = a.keys.iter().map(|k| (k.at, read(&k.body))).collect();
    keyed(&read(&a.body), &keys, TimeDelta::from_millis(ms))
}

/// Text a1 over output 1–2 s with a key at 1.5 s: size 48, black, other words.
fn keyed_text() -> Project {
    let mut p = project();
    let args = json!({"id": "a1", "at": 1.5, "size": 48, "color": "#000000ff", "text": "Б"});
    assert_eq!(run(&mut p, &set(args)), "аннотации ~1 a1");
    p
}

#[test]
fn a_change_at_a_moment_blends_sizes_and_colours_and_steps_the_words() {
    let p = keyed_text();
    let a = text(&p);
    let times: Vec<_> = a.keys.iter().map(|k| k.at.micros()).collect();
    assert_eq!(
        times,
        [0, 500_000],
        "the look at the start is kept in a key"
    );
    let mid = shown(&a, 250);
    assert_eq!((mid.size, mid.color), (40.0, Rgba([128, 128, 128, 255])));
    assert_eq!(mid.text, "Текст", "the words change at the key");
    assert_eq!(shown(&a, 500).text, "Б");
    assert_eq!(shown(&a, 900).size, 48.0, "the last key holds");
}

#[test]
fn a_key_added_between_takes_what_the_frame_shows() {
    let mut p = keyed_text();
    let add: AnnotKeyAdd = serde_json::from_value(json!({"id": "a1", "at": 1.2})).unwrap();
    run(&mut p, &add);
    let a = text(&p);
    let stored: TextBody = serde_json::from_value(Value::Object(a.keys[1].body.clone())).unwrap();
    let before = shown(&text(&keyed_text()), 200);
    assert_eq!(stored, before, "stored fields blend like the typed ones");
}

#[test]
fn without_a_moment_the_fields_change_everywhere() {
    let mut p = keyed_text();
    run(&mut p, &set(json!({"id": "a1", "bold": false})));
    let a = text(&p);
    assert!(!shown(&a, 0).bold && !shown(&a, 500).bold);
    assert_eq!(shown(&a, 500).size, 48.0, "the keyed size stays");
    let same = set(json!({"id": "a1", "at": 1.5, "size": 48}));
    assert_eq!(run(&mut p, &same), "", "the key holds it already");
    let error = set(json!({"id": "a2", "at": 1.5, "size": 48}))
        .apply(&mut p)
        .unwrap_err()
        .to_string();
    assert!(error.contains("не текстовая"), "{error}");
}
