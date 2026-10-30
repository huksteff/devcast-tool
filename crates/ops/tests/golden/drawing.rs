//! Task 5.35: drawing annotations over bar videos — pencil strokes, a straight line, a dot and an
//! eraser through them; draw-in midway; a turned see-through drawing under the zoom camera.

use std::error::Error;

use dc_core::{Command, CursorTracks, Project, Range, Rect, SourceTime, TimeDelta};
use dc_fx_annotations::{AnnotAdd, AnnotAnim};
use dc_fx_drawing::{DrawingAdd, DrawingSet};
use serde_json::{Value, json};

use crate::common::{TestResult, clip, size};

/// A 2 s 640×360 clip with a drawing annotation over 0.2–1.8 s in box `rect` (rotated by
/// `rotation`) with `strokes` given to `drawing add`.
fn project(rect: &str, rotation: f64, strokes: &[Value]) -> Result<Project, Box<dyn Error>> {
    let mut project = Project::default();
    clip(&mut project, "media/a.mp4", size(640, 360), Rect::FULL)?;
    let length = TimeDelta::from_secs_f64(2.0);
    for c in &mut project.clips {
        c.media.duration = length;
        c.trim = Range::new(SourceTime::ZERO, SourceTime::ZERO + length)?;
    }
    let add =
        json!({"kind": "drawing", "at": 0.2, "duration": 1.6, "rect": rect, "rotation": rotation});
    serde_json::from_value::<AnnotAdd>(add)?.apply(&mut project)?;
    for stroke in strokes {
        let mut args = json!({"id": "a1"});
        if let (Some(into), Value::Object(from)) = (args.as_object_mut(), stroke.clone()) {
            into.extend(from);
        }
        serde_json::from_value::<DrawingAdd>(args)?.apply(&mut project)?;
    }
    Ok(project)
}

fn check(name: &str, project: &Project) -> TestResult {
    crate::common::check(
        &format!("drawing/{name}"),
        project,
        CursorTracks::new(),
        1.0,
    )
}

/// A wavy pencil line, a thick see-through stroke, a red straight line and a dot, then an
/// eraser across the middle of all but the dot.
fn sketch() -> Vec<Value> {
    vec![
        json!({"points": "0,0.3,0.1,0.1,0.2,0.3,0.3,0.1,0.4,0.3,0.5,0.1,0.6,0.3,0.7,0.1,0.8,0.3,0.9,0.1,1,0.3", "width": 6}),
        json!({"points": "0.05,0.9,0.3,0.6,0.6,0.85,0.95,0.55", "color": "#facc1599", "width": 24}),
        json!({"points": "0.1,0.95,0.9,0.4", "color": "#ef4444ff", "width": 8}),
        json!({"points": "0.95,0.95", "color": "#ffffffff", "width": 20}),
        json!({"points": "0.45,0,0.5,1", "tool": "eraser", "width": 28}),
    ]
}

#[test]
fn golden_strokes_line_dot_and_eraser() -> TestResult {
    check("strokes", &project("0.1,0.15,0.8,0.7", 0.0, &sketch())?)
}

#[test]
fn golden_draw_in_midway() -> TestResult {
    let mut project = project("0.1,0.15,0.8,0.7", 0.0, &sketch())?;
    let anim = json!({"id": "a1", "enter": "draw", "enter_duration": 1.6});
    serde_json::from_value::<AnnotAnim>(anim)?.apply(&mut project)?;
    check("draw_in", &project)
}

#[test]
fn golden_turned_see_through_drawing_under_the_zoom_camera() -> TestResult {
    let mut project = project("0.15,0.3,0.4,0.4", 30.0, &sketch())?;
    let opacity = json!({"id": "a1", "opacity": 0.7});
    serde_json::from_value::<DrawingSet>(opacity)?.apply(&mut project)?;
    project.effects.insert(
        "zoom".to_owned(),
        json!({"zooms": [{
            "id": "z1", "media": "media/a.mp4", "range": {"start": 0, "end": 2_000_000},
            "scale": 1.6, "focus": {"x": 0.3, "y": 0.5}, "mode": "fixed"
        }]}),
    );
    check("turned", &project)
}
