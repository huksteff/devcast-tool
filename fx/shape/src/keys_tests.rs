use dc_core::{Command, Project, TimeDelta};
use dc_fx_annotations::{AnnotKeyAdd, Annotation, Annotations, Blend, keyed};
use dc_render::{GradientColors, Rgba};
use serde_json::{Value, json};

use crate::tests::{project, set};
use crate::{BoxPoint, Figure, ShapeBody};

fn run(p: &mut Project, command: &dyn Command) -> String {
    command.apply(p).unwrap().to_string()
}

fn shape(p: &Project) -> Annotation {
    p.state::<Annotations>().unwrap().annotations[0].clone()
}

/// The shape fields of `a` at key time `ms`, blended from the typed fields of its keys.
fn shown(a: &Annotation, ms: i64) -> ShapeBody {
    let read = |body: &serde_json::Map<String, Value>| -> ShapeBody {
        serde_json::from_value(Value::Object(body.clone())).unwrap()
    };
    let keys: Vec<_> = a.keys.iter().map(|k| (k.at, read(&k.body))).collect();
    keyed(&read(&a.body), &keys, TimeDelta::from_millis(ms))
}

/// Arrow a1 over output 1–2 s with a key at 1.5 s: tip moved up, thicker, red, a circle.
fn keyed_shape() -> Project {
    let mut p = project();
    let args = json!({"id": "a1", "at": 1.5, "to": "1,0", "width": 10,
        "color": "#ff0000ff", "glow": 20, "figure": "circle"});
    assert_eq!(run(&mut p, &set(args)), "аннотации ~1 a1");
    p
}

#[test]
fn a_change_at_a_moment_blends_points_and_colours_and_steps_the_figure() {
    let a = shape(&keyed_shape());
    let times: Vec<_> = a.keys.iter().map(|k| k.at.micros()).collect();
    assert_eq!(
        times,
        [0, 500_000],
        "the look at the start is kept in a key"
    );
    let mid = shown(&a, 250);
    assert_eq!(
        (mid.to, mid.width, mid.glow),
        (BoxPoint { x: 1.0, y: 0.25 }, 8.0, 10.0)
    );
    assert_eq!(mid.color, Rgba([0x92, 0x32, 0x76, 255]));
    assert_eq!(mid.figure, Figure::Arrow, "the figure changes at the key");
    assert_eq!(shown(&a, 500).figure, Figure::Circle);
    assert_eq!(shown(&a, 900).width, 10.0, "the last key holds");
}

#[test]
fn a_key_added_between_takes_what_the_frame_shows() {
    let mut p = keyed_shape();
    let add: AnnotKeyAdd = serde_json::from_value(json!({"id": "a1", "at": 1.2})).unwrap();
    run(&mut p, &add);
    let a = shape(&p);
    let stored: ShapeBody = serde_json::from_value(Value::Object(a.keys[1].body.clone())).unwrap();
    let before = shown(&shape(&keyed_shape()), 200);
    assert_eq!(stored, before, "stored fields blend like the typed ones");
}

#[test]
fn without_a_moment_the_fields_change_everywhere() {
    let mut p = keyed_shape();
    run(&mut p, &set(json!({"id": "a1", "opacity": 0.5})));
    let a = shape(&p);
    assert_eq!((shown(&a, 0).opacity, shown(&a, 500).opacity), (0.5, 0.5));
    assert_eq!(shown(&a, 500).width, 10.0, "the keyed thickness stays");
    let same = set(json!({"id": "a1", "at": 1.5, "width": 10}));
    assert_eq!(run(&mut p, &same), "", "the key holds it already");
}

#[test]
fn gradient_colours_blend_when_they_match_and_step_when_not() {
    let two = |a: u8, b: u8| GradientColors::two(Rgba([a, 0, 0, 255]), Rgba([b, 0, 0, 255]));
    let from = ShapeBody {
        colors: two(0, 100),
        ..ShapeBody::default()
    };
    let to = ShapeBody {
        colors: two(200, 0),
        ..ShapeBody::default()
    };
    assert_eq!(from.blend(&to, 0.5).colors, two(100, 50));
    let three = ShapeBody {
        colors: vec![Rgba([1, 1, 1, 255]); 3].try_into().unwrap(),
        ..ShapeBody::default()
    };
    assert_eq!(from.blend(&three, 0.5).colors, from.colors);
    assert_eq!(from.blend(&three, 1.0).colors, three.colors);
}
