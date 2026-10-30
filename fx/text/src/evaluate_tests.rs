use dc_core::{PixelSize, Rect, SourceTime};
use dc_fx_annotations::{Annotation, Kind, Placement};
use dc_render::{Align, Paint, PxRect, Rgba};
use serde_json::Map;

use crate::{TextAlign, TextBody, evaluate};

fn annotation(rect: Rect, rotation: f64) -> Annotation {
    Annotation {
        id: "a1".parse().unwrap(),
        kind: Kind::Text,
        media: "media/a.mp4".into(),
        range: dc_core::Range::new(SourceTime::ZERO, SourceTime::from_secs_f64(1.0)).unwrap(),
        track: 0,
        rect,
        rotation,
        enter: Default::default(),
        exit: Default::default(),
        body: Map::new(),
        keys: Vec::new(),
    }
}

fn placed(canvas: PixelSize, rotation: f64) -> Placement {
    Placement::of(
        &annotation(Rect::new(0.25, 0.5, 0.5, 0.25).unwrap(), rotation),
        canvas,
    )
}

const FULL_HD: PixelSize = PixelSize {
    width: 1920,
    height: 1080,
};

#[test]
fn the_box_is_canvas_px_and_sizes_follow_the_short_side() {
    let at = placed(FULL_HD, -30.0);
    assert_eq!(
        at.rect,
        PxRect {
            x: 480.0,
            y: 540.0,
            w: 960.0,
            h: 270.0
        }
    );
    assert_eq!((at.rotation, at.k), (-30.0, 1.0));
    let text = evaluate(&TextBody::default(), &at);
    assert_eq!((text.rect, text.rotation), (at.rect, -30.0));
    assert_eq!(
        (text.font.size, text.padding, text.line_height),
        (32.0, 8.0, 1.4)
    );
    assert_eq!((text.font.family.as_str(), text.font.bold), ("Inter", true));
    assert_eq!(text.align, Align::Center);
    assert!(text.plate.is_none(), "transparent background: no plate");
    let small = placed(
        PixelSize {
            width: 1440,
            height: 540,
        },
        0.0,
    );
    assert_eq!(small.k, 0.5);
    assert_eq!(evaluate(&TextBody::default(), &small).font.size, 16.0);
}

#[test]
fn a_background_makes_v1_plates_around_each_line() {
    let body = TextBody {
        text: "Две\nстроки".into(),
        size: 40.0,
        align: TextAlign::Right,
        underline: true,
        italic: true,
        color: Rgba([1, 2, 3, 255]),
        background: Rgba([0, 0, 0, 0xb0]),
        radius: 10.0,
        ..TextBody::default()
    };
    let at = placed(
        PixelSize {
            width: 3840,
            height: 2160,
        },
        0.0,
    );
    let text = evaluate(&body, &at);
    assert_eq!(text.runs.len(), 1);
    assert_eq!(
        (text.runs[0].text.as_str(), text.runs[0].color),
        ("Две\nстроки", body.color)
    );
    assert_eq!((text.font.size, text.padding), (80.0, 16.0));
    assert!(text.underline && text.font.italic);
    assert_eq!(text.align, Align::Right);
    let plate = text.plate.expect("a plate");
    assert_eq!(plate.paint, Paint::Solid(body.background));
    assert_eq!((plate.radius, plate.pad), (20.0, [16.0, 8.0]));
}

#[test]
fn reveal_types_whole_letters_and_hides_the_rest_in_place() {
    let body = TextBody {
        text: "Да 🎬!".into(),
        ..TextBody::default()
    };
    let at = |reveal: f64| {
        let placement = Placement {
            reveal,
            opacity: 0.25,
            ..placed(FULL_HD, 0.0)
        };
        evaluate(&body, &placement)
    };
    let runs = |reveal| {
        at(reveal)
            .runs
            .into_iter()
            .map(|r| (r.text, r.color.0[3]))
            .collect::<Vec<_>>()
    };
    assert_eq!(runs(1.0), [("Да 🎬!".to_owned(), 255)]);
    assert_eq!(
        runs(0.5),
        [("Да ".to_owned(), 255), ("🎬!".to_owned(), 0)],
        "5 letters: 2.5 rounds up, the emoji is one letter"
    );
    assert_eq!(runs(0.7), [("Да 🎬".to_owned(), 255), ("!".to_owned(), 0)]);
    assert_eq!(runs(0.0), [(String::new(), 255), ("Да 🎬!".to_owned(), 0)]);
    assert_eq!(at(0.5).runs[1].color.0[..3], body.color.0[..3]);
    assert_eq!(at(1.0).opacity, 0.25);
}
