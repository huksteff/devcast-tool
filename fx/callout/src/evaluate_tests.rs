use dc_core::{OutputTime, PixelSize, Project, TimeMap};
use dc_render::{Band, Camera, Layer, Paint, PxRect, Scene, Text};
use serde_json::json;

use super::{Anchor, evaluate};
use crate::Callout;
use crate::tests::{add, project, run, style};

const ANCHOR: Anchor = Anchor {
    tip: [400.0, 300.0],
    arrow: 28.0,
    k: 1.0,
};

/// One recording of 10 s with the callout on from 1 s saying «Привет».
fn on(look: serde_json::Value) -> Project {
    let mut p = project(&[10.0]);
    let mut look = look;
    look["text"] = json!("Привет");
    run(&mut p, &style(look));
    run(&mut p, &add(json!({"action": "on", "at": 1})));
    p
}

fn bubble(p: &Project, t: f64, anchor: Option<Anchor>, camera: Camera) -> Option<Text> {
    let state = p.state::<Callout>().unwrap();
    let mut scene = Scene::new(PixelSize {
        width: 1920,
        height: 1080,
    });
    scene.camera = camera;
    evaluate(
        &state,
        &TimeMap::build(p),
        OutputTime::from_secs_f64(t),
        anchor,
        &mut scene,
    );
    let mut layers = scene.painted();
    let found = layers.next().map(|(band, layer)| match (band, layer) {
        (Band::Cursor, Layer::Text(text)) => text.clone(),
        other => panic!("{other:?}"),
    });
    assert!(layers.next().is_none(), "one layer at most");
    found
}

fn at(p: &Project, t: f64) -> Option<Text> {
    bubble(p, t, Some(ANCHOR), Camera::IDENTITY)
}

#[test]
fn nothing_while_off_or_without_the_cursor() {
    let p = on(json!({}));
    assert!(at(&p, 0.5).is_none(), "before on");
    assert!(
        bubble(&p, 2.0, None, Camera::IDENTITY).is_none(),
        "no cursor point"
    );
    assert!(at(&p, 2.0).is_some());
}

#[test]
fn size_follows_the_arrow_with_ref_px_floors() {
    let p = on(json!({"size": 1.5}));
    let text = at(&p, 2.0).unwrap();
    assert_eq!(text.runs[0].text, "Привет");
    let font = 0.72 * 28.0 * 1.5;
    assert!(
        (f64::from(text.font.size) - font).abs() < 1e-3,
        "{}",
        text.font.size
    );
    let plate = text.plate.as_ref().unwrap();
    let pad = [0.55 * font, 0.32 * font];
    assert!((f64::from(plate.pad[0]) - pad[0]).abs() < 1e-3);
    assert!((f64::from(plate.pad[1]) - pad[1]).abs() < 1e-3);
    let height = font * 1.2 + 2.0 * pad[1];
    assert!(
        (f64::from(plate.radius) - height / 2.0).abs() < 1e-3,
        "a pill"
    );
    let beside = text.beside.as_ref().unwrap();
    assert_eq!(beside.point, [400.0, 300.0]);
    assert!(
        (beside.offset[0] - 0.60 * 28.0).abs() < 1e-4
            && (beside.offset[1] - 0.54 * 28.0).abs() < 1e-4
    );
    assert!((beside.margin - 16.0).abs() < 1e-4);

    let tiny = Anchor {
        arrow: 4.0,
        k: 2.0,
        ..ANCHOR
    };
    let small = bubble(&on(json!({})), 2.0, Some(tiny), Camera::IDENTITY).unwrap();
    assert!(
        (small.font.size - 32.0).abs() < 1e-4,
        "16 ref-px at k 2: {}",
        small.font.size
    );
    let pad = small.plate.unwrap().pad;
    assert!(
        (pad[0] - 20.0).abs() < 1e-4 && (pad[1] - 14.0).abs() < 1e-4,
        "{pad:?}"
    );
}

#[test]
fn solid_fill_or_a_gradient_flowing_by_output_time() {
    let solid = at(&on(json!({"background": "#112233ff"})), 2.0).unwrap();
    let plate = solid.plate.unwrap();
    assert_eq!(plate.paint, Paint::Solid("#112233ff".parse().unwrap()));
    assert!(plate.glow.is_none(), "glow 0 — none");

    let p = on(json!({"gradient": true, "colors": "#ff0000ff,#0000ffff", "glow": 20}));
    let phase = |t: f64| match at(&p, t).unwrap().plate.unwrap().paint {
        Paint::Gradient(g) => g.cycle.unwrap(),
        other => panic!("{other:?}"),
    };
    assert!((phase(1.2) - 0.5).abs() < 1e-4, "{}", phase(1.2));
    assert!(phase(2.4).abs() < 1e-4, "a full loop in 2.4 s");
    let glow = at(&p, 2.0).unwrap().plate.unwrap().glow.unwrap();
    assert_eq!(glow.color, "#ff0000ff".parse().unwrap(), "first colour");
    assert!((glow.sigma - 15.0).abs() < 1e-4, "{}", glow.sigma);
}

#[test]
fn glow_works_without_a_gradient() {
    let p = on(json!({"background": "#112233ff", "glow": 10}));
    let glow = at(&p, 2.0).unwrap().plate.unwrap().glow.unwrap();
    assert_eq!(glow.color, "#112233ff".parse().unwrap());
}

#[test]
fn edges_are_where_the_camera_shows() {
    let p = on(json!({}));
    let camera = Camera {
        scale: 2.0,
        x: -960.0,
        y: -540.0,
    };
    let text = bubble(&p, 2.0, Some(ANCHOR), camera).unwrap();
    assert_eq!(
        text.rect,
        PxRect {
            x: 480.0,
            y: 270.0,
            w: 960.0,
            h: 540.0
        }
    );
    assert_eq!(
        at(&p, 2.0).unwrap().rect,
        PxRect {
            x: 0.0,
            y: 0.0,
            w: 1920.0,
            h: 1080.0
        }
    );
}
