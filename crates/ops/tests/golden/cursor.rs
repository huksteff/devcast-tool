//! Tasks 5.20–5.23: the real cursor telemetry and pictures of the fixture `clicks.dcproj` over
//! bar videos.

use std::error::Error;

use dc_capture::input::Telemetry;
use dc_core::{CursorTracks, Project, Rect, SourceTime};
use dc_fx_cursor::{Cursor, CursorSettings, CursorState};

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

fn with(mut project: Project, settings: CursorSettings) -> Result<Project, Box<dyn Error>> {
    project.set_state::<Cursor>(&CursorState { settings })?;
    Ok(project)
}

/// Output second 0.4 s after click `n` (the fixture has no cuts).
fn after(clicks: &[SourceTime], n: usize) -> Result<f64, Box<dyn Error>> {
    Ok(clicks
        .get(n)
        .ok_or("fixture without that click")?
        .as_secs_f64()
        + 0.4)
}

#[test]
fn golden_arrow_at_rest_after_a_click() -> TestResult {
    let (project, tracks, clicks) = fixture()?;
    let project = with(project, CursorSettings::default())?;
    crate::common::check("cursor/arrow", &project, tracks, after(&clicks, 0)?)
}

#[test]
fn golden_large_hand_in_a_cropped_clip() -> TestResult {
    let (mut project, tracks, clicks) = fixture()?;
    let clip = project.clips.first_mut().ok_or("fixture without clips")?;
    clip.crop = Rect::new(0.5, 0.0, 0.5, 1.0)?;
    let settings = CursorSettings {
        size: 5.0,
        ..CursorSettings::default()
    };
    let project = with(project, settings)?;
    crate::common::check("cursor/hand_large", &project, tracks, after(&clicks, 1)?)
}
