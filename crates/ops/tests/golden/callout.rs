//! Task 5.25: the callout bubble at the real cursor of the fixture `clicks.dcproj` over bar videos.

use std::error::Error;

use dc_capture::input::Telemetry;
use dc_core::{Command, CursorTracks, Project, SourceTime};
use dc_fx_callout::{CalloutAdd, CalloutStyle};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::common::{TestResult, clicks_fixture};

/// The fixture project, its cursor telemetry and the moments of its clicks.
fn fixture() -> Result<(Project, CursorTracks, Vec<SourceTime>), Box<dyn Error>> {
    let root = clicks_fixture();
    let project: Project =
        serde_json::from_str(&std::fs::read_to_string(root.join("project.json"))?)?;
    let media = &project.clips.first().ok_or("fixture without clips")?.media;
    let dir = root.join("media/recording-1");
    let track = Telemetry::read(&dir)?.cursor_track(&dir, media)?;
    let clicks = track.clicks.iter().map(|c| c.at).collect();
    let tracks = CursorTracks::from([(media.path.clone(), track)]);
    Ok((project, tracks, clicks))
}

fn apply<C: Command + DeserializeOwned>(project: &mut Project, args: Value) -> TestResult {
    serde_json::from_value::<C>(args)?.apply(project)?;
    Ok(())
}

/// The callout on from the start with `look`, then typing `text` for a second from `typing`
/// (output seconds; the fixture has no cuts); the frame at `at`.
fn check(name: &str, look: Value, text: &str, typing: f64, at: f64) -> TestResult {
    let (mut project, cursors, _) = fixture()?;
    apply::<CalloutStyle>(&mut project, look)?;
    apply::<CalloutAdd>(&mut project, json!({"action": "on", "at": 0}))?;
    apply::<CalloutAdd>(
        &mut project,
        json!({"action": "text", "at": typing, "text": text, "duration": 1}),
    )?;
    crate::common::check(&format!("callout/{name}"), &project, cursors, at)
}

/// Output second `secs` after click `n`.
fn after(n: usize, secs: f64) -> Result<f64, Box<dyn Error>> {
    let (_, _, clicks) = fixture()?;
    Ok(clicks
        .get(n)
        .ok_or("fixture without that click")?
        .as_secs_f64()
        + secs)
}

#[test]
fn golden_bubble_by_the_arrow() -> TestResult {
    // Click 0 at (200, 150) of the window: the bubble right and below the tip.
    let at = after(0, 0.4)?;
    check("plain", json!({}), "Нажми сюда", at - 1.5, at)
}

#[test]
fn golden_text_half_typed() -> TestResult {
    let at = after(2, 0.3)?;
    check("typing", json!({}), "Печатается по буквам", at - 0.5, at)
}

#[test]
fn golden_gradient_with_glow() -> TestResult {
    let at = after(3, 0.4)?;
    let look = json!({"gradient": true, "colors": "#14b8c4ff,#a855f7ff,#f59e0bff", "glow": 20, "size": 1.5});
    check("gradient", look, "Градиент и свечение", at - 1.5, at)
}

#[test]
fn golden_bubble_turns_at_the_right_edge() -> TestResult {
    // Click 1 at (1080, 150) of the 1280 px window: the long bubble goes left of the tip.
    let at = after(1, 0.4)?;
    check(
        "edge",
        json!({"text_color": "#111827ff", "background": "#fde047ff"}),
        "У края облачко перескакивает влево",
        at - 1.5,
        at,
    )
}
