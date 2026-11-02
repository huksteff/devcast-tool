//! Task 5.30: annotation animations halfway — a pop on the way in, a turned slide on the way
//! out, a typewriter with its plates growing with the typed letters.

use dc_core::{Command, Project};
use dc_fx_annotations::AnnotAnim;
use serde_json::{Value, json};

use crate::common::TestResult;
use crate::text::project;

fn animate(project: &mut Project, args: Value) -> TestResult {
    let mut full = json!({"id": "a1"});
    if let (Some(into), Value::Object(from)) = (full.as_object_mut(), args) {
        into.extend(from);
    }
    serde_json::from_value::<AnnotAnim>(full)?.apply(project)?;
    Ok(())
}

fn check(name: &str, project: &Project, at: f64) -> TestResult {
    crate::common::check(
        &format!("motion/{name}"),
        project,
        dc_core::CursorTracks::new(),
        at,
    )
}

/// The text is shown 0.2–1.8 s; 0.6 s is halfway into a 0.8 s pop: 0.93 of the size, half
/// transparent.
#[test]
fn golden_pop_halfway_in() -> TestResult {
    let mut project = project(
        json!({"rect": "0.25,0.3,0.5,0.4"}),
        json!({"text": "Нажми «Экспорт»", "background": "#2563ebff", "size": 48}),
    )?;
    animate(&mut project, json!({"enter": "pop", "enter_duration": 0.8}))?;
    check("pop", &project, 0.6)
}

/// 1.4 s is halfway through a 0.8 s exit of a slide_left: it keeps moving, 48 px at 1080 right of
/// the turned box, half transparent.
#[test]
fn golden_turned_slide_halfway_out() -> TestResult {
    let mut project = project(
        json!({"rect": "0.3,0.3,0.4,0.3", "rotation": 15}),
        json!({"text": "Уезжаю", "background": "#e11d48e0", "size": 56}),
    )?;
    animate(
        &mut project,
        json!({"exit": "slide_left", "exit_duration": 0.8}),
    )?;
    check("slide", &project, 1.4)
}

/// 0.7 s is halfway into a 1 s typewriter: 12 of 24 letters, the plate only under them.
#[test]
fn golden_typewriter_halfway() -> TestResult {
    let mut project = project(
        json!({"rect": "0.15,0.25,0.7,0.5"}),
        json!({"text": "Новая функция\nв два ряда", "background": "#0f172acc",
            "align": "left", "size": 40}),
    )?;
    animate(
        &mut project,
        json!({"enter": "typewriter", "enter_duration": 1}),
    )?;
    check("typewriter", &project, 0.7)
}
