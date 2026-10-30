//! Task 5.31: keyframes of a text annotation — halfway between two keys (box, size, colours
//! blended) and a moved text turning on its way.

use dc_core::{Command, Project};
use dc_fx_annotations::AnnotSet;
use dc_fx_text::TextSet;
use serde_json::json;

use crate::common::TestResult;
use crate::text::project;

fn check(name: &str, project: &Project, at: f64) -> TestResult {
    crate::common::check(
        &format!("keyframes/{name}"),
        project,
        dc_core::CursorTracks::new(),
        at,
    )
}

/// The text is shown 0.2–1.8 s; a change keyed at 1.6 s starts from the look at 0.2 s, so
/// 0.9 s is halfway: the box at 0.35,0.4, size 48, the plate between blue and red.
#[test]
fn golden_halfway_between_two_keys() -> TestResult {
    let mut project = project(
        json!({"rect": "0.1,0.3,0.3,0.2"}),
        json!({"text": "Сюда", "background": "#2563ebff", "size": 32}),
    )?;
    serde_json::from_value::<AnnotSet>(json!({"id": "a1", "at": 1.6, "x": 0.6, "y": 0.5}))?
        .apply(&mut project)?;
    serde_json::from_value::<TextSet>(
        json!({"id": "a1", "at": 1.6, "size": 64, "background": "#e11d48ff"}),
    )?
    .apply(&mut project)?;
    check("between", &project, 0.9)
}

/// Keys at 0.2 s (upright, top left) and 1.4 s (30° clockwise, wider, lower right); 1.1 s is
/// three quarters of the way, smoothstep 0.84: turned about 25° with its plate.
#[test]
fn golden_turned_on_its_way() -> TestResult {
    let mut project = project(
        json!({"rect": "0.05,0.1,0.35,0.2"}),
        json!({"text": "Поворот по ключам", "background": "#0f172acc", "size": 36}),
    )?;
    let to = json!({"id": "a1", "at": 1.4, "rect": "0.45,0.55,0.45,0.25", "rotation": 30});
    serde_json::from_value::<AnnotSet>(to)?.apply(&mut project)?;
    check("turned", &project, 1.1)
}
