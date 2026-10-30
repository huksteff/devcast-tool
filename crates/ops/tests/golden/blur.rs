//! Task 5.36: blur annotations over bar videos — a pure blur, a half tint and a solid «censor»
//! fill side by side over a figure below them, and turned regions under the zoom camera mid-pop.

use std::error::Error;

use dc_core::{Command, CursorTracks, Project, Range, Rect, SourceTime, TimeDelta};
use dc_fx_annotations::{AnnotAdd, AnnotAnim};
use dc_fx_blur::BlurSet;
use dc_fx_shape::ShapeSet;
use serde_json::{Value, json};

use crate::common::{TestResult, clip, size};

/// A 2 s 640×360 clip with a white diagonal line on track 0 over the whole frame and a blur
/// annotation on track 1 over 0.2–1.8 s for each `(place, look)`: `place` goes to `annot add`,
/// `look` to `blur set`.
fn project(regions: &[(Value, Value)]) -> Result<Project, Box<dyn Error>> {
    let mut project = Project::default();
    clip(&mut project, "media/a.mp4", size(640, 360), Rect::FULL)?;
    let length = TimeDelta::from_secs_f64(2.0);
    for c in &mut project.clips {
        c.media.duration = length;
        c.trim = Range::new(SourceTime::ZERO, SourceTime::ZERO + length)?;
    }
    let line = json!({"kind": "shape", "at": 0.2, "duration": 1.6, "rect": "0,0,1,1"});
    serde_json::from_value::<AnnotAdd>(line)?.apply(&mut project)?;
    let look = json!({"id": "a1", "figure": "line", "from": "0.02,0.9", "to": "0.98,0.1",
        "color": "#ffffffff", "width": 24});
    serde_json::from_value::<ShapeSet>(look)?.apply(&mut project)?;
    for (i, (place, look)) in regions.iter().enumerate() {
        let mut add = json!({"kind": "blur", "at": 0.2, "duration": 1.6, "track": 1});
        merge(&mut add, place.clone());
        serde_json::from_value::<AnnotAdd>(add)?.apply(&mut project)?;
        let mut set = json!({"id": format!("a{}", i + 2)});
        merge(&mut set, look.clone());
        serde_json::from_value::<BlurSet>(set)?.apply(&mut project)?;
    }
    Ok(project)
}

fn merge(into: &mut Value, from: Value) {
    if let (Some(into), Value::Object(from)) = (into.as_object_mut(), from) {
        into.extend(from);
    }
}

fn check(name: &str, project: &Project, at: f64) -> TestResult {
    crate::common::check(&format!("blur/{name}"), project, CursorTracks::new(), at)
}

/// Left to right: a pure blur (σ 20 at 1080), a half black tint, a solid red fill with round
/// corners.
#[test]
fn golden_pure_tinted_and_censored() -> TestResult {
    let project = project(&[
        (json!({"rect": "0.04,0.15,0.28,0.7"}), json!({})),
        (
            json!({"rect": "0.36,0.15,0.28,0.7"}),
            json!({"tint": "#00000080", "strength": 30}),
        ),
        (
            json!({"rect": "0.68,0.15,0.28,0.7"}),
            json!({"tint": "#dc2626ff", "radius": 40}),
        ),
    ])?;
    check("regions", &project, 1.0)
}

/// Turned regions under a ×1.6 zoom: one held, one midway through its pop and fade.
#[test]
fn golden_turned_under_the_zoom_camera() -> TestResult {
    let mut project = project(&[
        (
            json!({"rect": "0.12,0.3,0.3,0.3", "rotation": 30}),
            json!({"radius": 24}),
        ),
        (
            json!({"rect": "0.45,0.45,0.35,0.2", "rotation": -15}),
            json!({"tint": "#1d4ed8a0"}),
        ),
    ])?;
    let anim = json!({"id": "a3", "enter": "pop", "enter_duration": 1.6});
    serde_json::from_value::<AnnotAnim>(anim)?.apply(&mut project)?;
    project.effects.insert(
        "zoom".to_owned(),
        json!({"zooms": [{
            "id": "z1", "media": "media/a.mp4", "range": {"start": 0, "end": 2_000_000},
            "scale": 1.6, "focus": {"x": 0.4, "y": 0.5}, "mode": "fixed"
        }]}),
    );
    check("turned", &project, 1.0)
}
