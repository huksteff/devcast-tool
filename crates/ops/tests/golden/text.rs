//! Task 5.32: text annotations over bar videos — plain, turned with a plate, styled (font,
//! italic, underline, left-aligned lines) and under the zoom camera.

use std::error::Error;

use dc_core::{Command, CursorTracks, Project, Range, Rect, SourceTime, TimeDelta};
use dc_fx_annotations::AnnotAdd;
use dc_fx_text::TextSet;
use serde_json::{Value, json};

use crate::common::{TestResult, clip, size};

/// A 2 s 640×360 clip with a text annotation over 0.2–1.8 s placed by `place` and looking as
/// `look` says.
pub(crate) fn project(place: Value, look: Value) -> Result<Project, Box<dyn Error>> {
    let mut project = Project::default();
    clip(&mut project, "media/a.mp4", size(640, 360), Rect::FULL)?;
    let length = TimeDelta::from_secs_f64(2.0);
    for c in &mut project.clips {
        c.media.duration = length;
        c.trim = Range::new(SourceTime::ZERO, SourceTime::ZERO + length)?;
    }
    let mut add = json!({"kind": "text", "at": 0.2, "duration": 1.6});
    merge(&mut add, place);
    serde_json::from_value::<AnnotAdd>(add)?.apply(&mut project)?;
    let mut set = json!({"id": "a1"});
    merge(&mut set, look);
    serde_json::from_value::<TextSet>(set)?.apply(&mut project)?;
    Ok(project)
}

fn merge(into: &mut Value, from: Value) {
    if let (Some(into), Value::Object(from)) = (into.as_object_mut(), from) {
        into.extend(from);
    }
}

fn check(name: &str, project: &Project) -> TestResult {
    crate::common::check(&format!("text/{name}"), project, CursorTracks::new(), 1.0)
}

#[test]
fn golden_plain_text_in_the_default_look() -> TestResult {
    let project = project(
        json!({"rect": "0.2,0.35,0.6,0.3"}),
        json!({"text": "Нажми «Экспорт»"}),
    )?;
    check("plain", &project)
}

#[test]
fn golden_turned_text_on_a_plate() -> TestResult {
    let project = project(
        json!({"rect": "0.25,0.3,0.5,0.4", "rotation": -15}),
        json!({"text": "Новая функция 🎬\nв два ряда", "background": "#e11d48e0",
            "radius": 12, "size": 40}),
    )?;
    check("turned", &project)
}

#[test]
fn golden_styled_text_left_aligned() -> TestResult {
    let project = project(
        json!({"rect": "0.05,0.1,0.55,0.5"}),
        json!({"text": "Шаг 1: открой настройки\nШаг 2: включи запись",
            "font": "PT Serif", "size": 36, "bold": false, "italic": true, "underline": true,
            "align": "left", "color": "#fde047ff", "background": "#0f172acc", "radius": 0}),
    )?;
    check("styled", &project)
}

#[test]
fn golden_turned_text_under_the_zoom_camera() -> TestResult {
    let mut project = project(
        json!({"rect": "0.1,0.6,0.4,0.25", "rotation": 20}),
        json!({"text": "Здесь", "background": "#2563ebff", "size": 48}),
    )?;
    project.effects.insert(
        "zoom".to_owned(),
        json!({"zooms": [{
            "id": "z1", "media": "media/a.mp4", "range": {"start": 0, "end": 2_000_000},
            "scale": 2.0, "focus": {"x": 0.3, "y": 0.7}, "mode": "fixed"
        }]}),
    );
    check("zoom", &project)
}
