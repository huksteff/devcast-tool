use dc_core::project::{Media, MediaKind};
use dc_core::{Clip, Command, History, PixelSize, Project, Range, Rect, SourceTime, TimeDelta};
use dc_fx_annotations::{AnnotAdd, Annotations, AnnotationsState};
use dc_render::Rgba;
use serde_json::{Value, json};

use crate::{BoxPoint, Figure, HIGHLIGHT_OPACITY, ShapeBody, ShapeSet};

fn pt(x: f64, y: f64) -> BoxPoint {
    BoxPoint { x, y }
}

/// One 10 s recording with shape annotation a1 and text annotation a2, both 1–2 s.
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
    });
    for kind in ["shape", "text"] {
        let add: AnnotAdd = serde_json::from_value(json!({"kind": kind, "at": 1})).unwrap();
        add.apply(&mut p).unwrap();
    }
    p
}

pub(crate) fn set(args: Value) -> ShapeSet {
    serde_json::from_value(args).unwrap()
}

fn state(p: &Project) -> AnnotationsState {
    p.state::<Annotations>().unwrap()
}

fn body(p: &Project) -> ShapeBody {
    state(p).annotations[0].body().unwrap()
}

#[test]
fn a_new_shape_is_the_blue_v1_arrow_across_its_box() {
    let b = body(&project());
    assert_eq!(b, ShapeBody::default());
    assert_eq!(b.figure, Figure::Arrow);
    assert_eq!((b.from, b.to), (pt(0.0, 0.5), pt(1.0, 0.5)));
    assert_eq!(b.color, Rgba([0x25, 0x63, 0xeb, 255]));
    assert_eq!(b.fill.0[3], 0, "no fill");
    assert!(b.outline && !b.gradient);
    assert_eq!((b.glow, b.opacity, b.cycle), (0.0, 1.0, 2.4));
}

#[test]
fn set_changes_only_the_given_fields() {
    let mut p = project();
    let args = json!({"id": "a1", "figure": "line", "from": "0, 1", "to": "1,0.25",
        "width": 12, "gradient": true, "colors": "#ff0000ff,#00ff00ff,#0000ffff",
        "angle": 90, "cycle": 0, "glow": 14});
    assert_eq!(
        set(args).apply(&mut p).unwrap().to_string(),
        "аннотации ~1 a1"
    );
    let b = body(&p);
    assert_eq!(b.figure, Figure::Line);
    assert_eq!((b.from, b.to, b.width), (pt(0.0, 1.0), pt(1.0, 0.25), 12.0));
    assert!(b.gradient);
    assert_eq!(b.colors.colors().len(), 3);
    assert_eq!((b.angle, b.cycle, b.glow), (90.0, 0.0, 14.0));
    assert_eq!(b.color, ShapeBody::default().color, "untouched");
    let same = set(json!({"id": "a1", "width": 12})).apply(&mut p).unwrap();
    assert_eq!(same.to_string(), "", "nothing changed");
}

#[test]
fn a_highlight_comes_filled_and_see_through_and_leaves_as_an_outline() {
    let mut p = project();
    let red = "#ef4444ff";
    set(json!({"id": "a1", "figure": "highlight", "color": red}))
        .apply(&mut p)
        .unwrap();
    let b = body(&p);
    assert_eq!(
        b.fill,
        Rgba([0xef, 0x44, 0x44, 255]),
        "in the colour given with it"
    );
    assert_eq!(b.color, Rgba([0xef, 0x44, 0x44, 255]));
    assert!(!b.outline);
    assert_eq!(b.opacity, HIGHLIGHT_OPACITY);
    set(json!({"id": "a1", "figure": "rectangle", "opacity": 0.5}))
        .apply(&mut p)
        .unwrap();
    let b = body(&p);
    assert!(b.outline && b.fill.0[3] == 0);
    assert_eq!(b.opacity, 0.5, "given in the same command");
    set(json!({"id": "a1", "fill": "#ffffff80"}))
        .apply(&mut p)
        .unwrap();
    set(json!({"id": "a1", "figure": "ellipse"}))
        .apply(&mut p)
        .unwrap();
    assert_eq!(
        body(&p).fill,
        Rgba([255, 255, 255, 128]),
        "kept between closed figures"
    );
}

#[test]
fn set_rejects_bad_values_and_other_kinds() {
    let mut p = project();
    for (args, message) in [
        (json!({"id": "a1", "width": 0.5}), "от 1 до 64"),
        (json!({"id": "a1", "width": 65}), "от 1 до 64"),
        (json!({"id": "a1", "radius": -1}), "от 0 до 540"),
        (json!({"id": "a1", "glow": 41}), "от 0 до 40"),
        (json!({"id": "a1", "opacity": 1.5}), "от 0 до 1"),
        (json!({"id": "a1", "angle": 400}), "от -360 до 360"),
        (json!({"id": "a1", "cycle": 0.1}), "или 0"),
        (json!({"id": "a1", "from": "0.5"}), "x,y в долях бокса"),
        (json!({"id": "a1", "to": "1.5,0"}), "от 0 до 1"),
        (json!({"id": "a1", "to": "a,b"}), "например 0,0.5"),
        (json!({"id": "a1", "colors": "#ff0000ff"}), "от 2 до 4"),
        (json!({"id": "a1", "colors": "red,blue"}), "#rrggbbaa"),
        (json!({"id": "a2", "width": 4}), "не фигура"),
        (json!({"id": "a9", "width": 4}), "a9"),
    ] {
        let before = p.clone();
        let error = set(args.clone()).apply(&mut p).unwrap_err().to_string();
        assert!(error.contains(message), "{args}: {error}");
        assert_eq!(p, before, "{args}");
    }
    let unknown = serde_json::from_value::<ShapeSet>(json!({"id": "a1", "figure": "star"}));
    assert!(unknown.is_err());
}

#[test]
fn undo_redo_and_the_project_file_keep_the_shape() {
    let mut p = project();
    let start = p.clone();
    let mut history = History::new(10);
    history
        .apply(&mut p, &set(json!({"id": "a1", "figure": "circle"})))
        .unwrap();
    history
        .apply(
            &mut p,
            &set(json!({"id": "a1", "gradient": true, "glow": 10})),
        )
        .unwrap();
    let last = p.clone();
    while history.undo(&mut p).is_some() {}
    assert_eq!(p, start);
    while history.redo(&mut p).is_some() {}
    assert_eq!(p, last);
    let loaded: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(body(&loaded).figure, Figure::Circle);
    assert!(body(&loaded).gradient);
    let stored = serde_json::to_value(&state(&loaded).annotations[0]).unwrap();
    assert_eq!(stored["body"]["colors"], json!(["#ff4d9dff", "#22d3eeff"]));
}

#[test]
fn ops_follow_the_rules() {
    let ops = crate::ops();
    assert_eq!(ops.len(), 1);
    let op = &ops[0];
    assert_eq!(op.name, "shape set");
    assert!(op.example.starts_with(op.name));
    let schema = serde_json::to_value((op.schema)()).unwrap();
    assert_eq!(schema["additionalProperties"], false);
}
