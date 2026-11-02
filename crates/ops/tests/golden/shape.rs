//! Task 5.33: shape annotations over bar videos — every figure, gradients with glow, draw-in
//! midway, turned figures under the zoom camera.

use std::error::Error;

use dc_core::{Command, CursorTracks, Project, Range, Rect, SourceTime, TimeDelta};
use dc_fx_annotations::{AnnotAdd, AnnotAnim};
use dc_fx_shape::ShapeSet;
use serde_json::{Value, json};

use crate::common::{TestResult, clip, size};

/// A 2 s 640×360 clip with a shape annotation over 0.2–1.8 s for each `(place, look)`: `place`
/// goes to `annot add`, `look` to `shape set`.
fn project(shapes: &[(Value, Value)]) -> Result<Project, Box<dyn Error>> {
    let mut project = Project::default();
    clip(&mut project, "media/a.mp4", size(640, 360), Rect::FULL)?;
    let length = TimeDelta::from_secs_f64(2.0);
    for c in &mut project.clips {
        c.media.duration = length;
        c.trim = Range::new(SourceTime::ZERO, SourceTime::ZERO + length)?;
    }
    for (i, (place, look)) in shapes.iter().enumerate() {
        let mut add = json!({"kind": "shape", "at": 0.2, "duration": 1.6});
        merge(&mut add, place.clone());
        serde_json::from_value::<AnnotAdd>(add)?.apply(&mut project)?;
        let mut set = json!({"id": format!("a{}", i + 1)});
        merge(&mut set, look.clone());
        serde_json::from_value::<ShapeSet>(set)?.apply(&mut project)?;
    }
    Ok(project)
}

fn merge(into: &mut Value, from: Value) {
    if let (Some(into), Value::Object(from)) = (into.as_object_mut(), from) {
        into.extend(from);
    }
}

fn check(name: &str, project: &Project) -> TestResult {
    crate::common::check(&format!("shape/{name}"), project, CursorTracks::new(), 1.0)
}

/// Six figures in a 3×2 grid.
fn figures() -> Vec<(Value, Value)> {
    let cell = |col: f64, row: f64| json!({"rect": format!("{},{},0.28,0.36", 0.04 + col * 0.32, 0.08 + row * 0.46)});
    vec![
        (
            cell(0.0, 0.0),
            json!({"figure": "arrow", "tail": "0,1", "tip": "1,0.2"}),
        ),
        (
            cell(1.0, 0.0),
            json!({"figure": "rectangle", "radius": 16, "color": "#f97316ff"}),
        ),
        (
            cell(2.0, 0.0),
            json!({"figure": "ellipse", "fill": "#22c55e80", "color": "#16a34aff"}),
        ),
        (
            cell(0.0, 1.0),
            json!({"figure": "circle", "width": 3, "color": "#ffffffff"}),
        ),
        (
            cell(1.0, 1.0),
            json!({"figure": "line", "tail": "0,1", "tip": "1,0", "color": "#e11d48ff", "width": 10}),
        ),
        (
            cell(2.0, 1.0),
            json!({"figure": "highlight", "color": "#facc15ff"}),
        ),
    ]
}

#[test]
fn golden_every_figure() -> TestResult {
    check("figures", &project(&figures())?)
}

#[test]
fn golden_gradients_with_glow() -> TestResult {
    let project = project(&[
        (
            json!({"rect": "0.08,0.2,0.4,0.6"}),
            json!({"figure": "rectangle", "outline": false, "gradient": true, "radius": 24,
                "colors": "#ff4d9dff,#a855f7ff,#22d3eeff", "angle": 30, "glow": 16}),
        ),
        (
            json!({"rect": "0.55,0.25,0.4,0.5"}),
            json!({"figure": "arrow", "tail": "0,0.8", "tip": "1,0.2", "width": 12,
                "gradient": true, "cycle": 0, "glow": 10}),
        ),
    ])?;
    check("gradient_glow", &project)
}

#[test]
fn golden_draw_in_midway() -> TestResult {
    let mut project = project(&figures())?;
    for i in 1..=6 {
        let anim = json!({"id": format!("a{i}"), "enter": "draw", "enter_duration": 1.6});
        serde_json::from_value::<AnnotAnim>(anim)?.apply(&mut project)?;
    }
    check("draw_in", &project)
}

#[test]
fn golden_turned_figures_under_the_zoom_camera() -> TestResult {
    let mut project = project(&[
        (
            json!({"rect": "0.15,0.45,0.3,0.3", "rotation": 25}),
            json!({"figure": "rectangle", "fill": "#2563eb60", "radius": 10}),
        ),
        (
            json!({"rect": "0.12,0.32,0.3,0.15", "rotation": -30}),
            json!({"figure": "arrow", "color": "#f43f5eff", "width": 8}),
        ),
    ])?;
    project.effects.insert(
        "zoom".to_owned(),
        json!({"zooms": [{
            "id": "z1", "media": "media/a.mp4", "range": {"start": 0, "end": 2_000_000},
            "scale": 1.6, "focus": {"x": 0.3, "y": 0.5}, "mode": "fixed"
        }]}),
    );
    check("turned", &project)
}
