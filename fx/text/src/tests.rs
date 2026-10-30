use dc_core::project::{Media, MediaKind};
use dc_core::{Clip, Command, History, PixelSize, Project, Range, Rect, SourceTime, TimeDelta};
use dc_fx_annotations::{AnnotAdd, Annotations, AnnotationsState};
use dc_render::Rgba;
use serde_json::{Value, json};

use crate::{TextAlign, TextBody, TextSet};

/// One 10 s recording with text annotation a1 and shape annotation a2.
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
    for kind in ["text", "shape"] {
        let add: AnnotAdd = serde_json::from_value(json!({"kind": kind, "at": 1})).unwrap();
        add.apply(&mut p).unwrap();
    }
    p
}

pub(crate) fn set(args: Value) -> TextSet {
    serde_json::from_value(args).unwrap()
}

fn state(p: &Project) -> AnnotationsState {
    p.state::<Annotations>().unwrap()
}

fn body(p: &Project) -> TextBody {
    state(p).annotations[0].body().unwrap()
}

#[test]
fn a_new_text_annotation_has_the_v1_look_in_inter() {
    let b = body(&project());
    assert_eq!(b, TextBody::default());
    assert_eq!(
        (b.text.as_str(), b.font.as_str(), b.size),
        ("Текст", "Inter", 32.0)
    );
    assert!(b.bold && !b.italic && !b.underline);
    assert_eq!((b.align, b.color), (TextAlign::Center, Rgba([255; 4])));
    assert_eq!((b.background.0[3], b.radius), (0, 4.0));
}

#[test]
fn set_changes_only_the_given_fields() {
    let mut p = project();
    let diff = set(
        json!({"id": "a1", "text": "Нажми сюда 👉\nи жди", "font": "PT Serif",
        "size": 48, "italic": true, "underline": true, "align": "left",
        "background": "#000000b0", "radius": 12}),
    )
    .apply(&mut p)
    .unwrap();
    assert_eq!(diff.to_string(), "аннотации ~1 a1");
    let b = body(&p);
    assert_eq!(b.text, "Нажми сюда 👉\nи жди");
    assert_eq!(
        (b.font.as_str(), b.size, b.radius),
        ("PT Serif", 48.0, 12.0)
    );
    assert!(b.bold && b.italic && b.underline);
    assert_eq!(b.align, TextAlign::Left);
    assert_eq!(b.background, Rgba([0, 0, 0, 0xb0]));
    assert_eq!(b.color, Rgba([255; 4]), "untouched");
    let same = set(json!({"id": "a1", "size": 48})).apply(&mut p).unwrap();
    assert_eq!(same.to_string(), "", "nothing changed");
}

#[test]
fn set_rejects_bad_values_and_other_kinds() {
    let mut p = project();
    for (args, message) in [
        (json!({"id": "a1", "size": 11}), "от 12 до 128"),
        (json!({"id": "a1", "size": 129}), "от 12 до 128"),
        (json!({"id": "a1", "radius": -1}), "от 0 до 64"),
        (json!({"id": "a1", "font": "  "}), "шрифт не указан"),
        (json!({"id": "a1", "text": "a\tb"}), "управляющие символы"),
        (
            json!({"id": "a1", "text": "я".repeat(2001)}),
            "длиннее 2000",
        ),
        (json!({"id": "a2", "text": "x"}), "не текстовая"),
        (json!({"id": "a9", "text": "x"}), "a9"),
    ] {
        let before = p.clone();
        let error = set(args.clone()).apply(&mut p).unwrap_err().to_string();
        assert!(error.contains(message), "{args}: {error}");
        assert_eq!(p, before, "{args}");
    }
}

#[test]
fn undo_redo_and_the_project_file_keep_the_text() {
    let mut p = project();
    let start = p.clone();
    let mut history = History::new(10);
    history
        .apply(
            &mut p,
            &set(json!({"id": "a1", "text": "Раз", "bold": false})),
        )
        .unwrap();
    history
        .apply(&mut p, &set(json!({"id": "a1", "color": "#ffcc00ff"})))
        .unwrap();
    let last = p.clone();
    while history.undo(&mut p).is_some() {}
    assert_eq!(p, start);
    while history.redo(&mut p).is_some() {}
    assert_eq!(p, last);
    let loaded: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(body(&loaded).text, "Раз");
    assert_eq!(body(&loaded).color, Rgba([0xff, 0xcc, 0, 0xff]));
}

#[test]
fn ops_follow_the_rules() {
    let ops = crate::ops();
    assert_eq!(ops.len(), 1);
    let op = &ops[0];
    assert_eq!(op.name, "text set");
    assert!(op.example.starts_with(op.name));
    let schema = serde_json::to_value((op.schema)()).unwrap();
    assert_eq!(schema["additionalProperties"], false);
}
