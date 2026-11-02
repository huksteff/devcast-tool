//! Tasks 5.20–5.23: the real cursor telemetry and pictures of the fixture `clicks.devcast` over
//! bar videos.

use std::error::Error;

use dc_capture::input::Telemetry;
use dc_core::{CursorTracks, Project, Rect, SourceTime};
use dc_fx_cursor::{ClickChange, ClickKind, ClickStyle, Cursor, CursorSettings, CursorState};
use dc_render::Rgba;

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

fn with(project: Project, settings: CursorSettings) -> Result<Project, Box<dyn Error>> {
    with_clicks(project, settings, Vec::new())
}

fn with_clicks(
    mut project: Project,
    settings: CursorSettings,
    clicks: Vec<ClickChange>,
) -> Result<Project, Box<dyn Error>> {
    project.set_state::<Cursor>(&CursorState { settings, clicks })?;
    Ok(project)
}

/// Every click shows `kind`.
fn clicking(kind: ClickKind) -> CursorSettings {
    CursorSettings {
        click: ClickStyle {
            kind,
            ..ClickStyle::default()
        },
        ..CursorSettings::default()
    }
}

/// Output second `secs` after click `n` (the fixture has no cuts).
fn after(clicks: &[SourceTime], n: usize, secs: f64) -> Result<f64, Box<dyn Error>> {
    Ok(clicks
        .get(n)
        .ok_or("fixture without that click")?
        .as_secs_f64()
        + secs)
}

#[test]
fn golden_arrow_at_rest_after_a_click() -> TestResult {
    let (project, tracks, clicks) = fixture()?;
    let project = with(project, CursorSettings::default())?;
    crate::common::check("cursor/arrow", &project, tracks, after(&clicks, 0, 0.4)?)
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
    crate::common::check(
        "cursor/hand_large",
        &project,
        tracks,
        after(&clicks, 1, 0.4)?,
    )
}

#[test]
fn golden_blurred_tilted_cursor_on_its_way() -> TestResult {
    let (project, tracks, clicks) = fixture()?;
    let settings = CursorSettings {
        blur: 1.0,
        sway: 1.0,
        ..CursorSettings::default()
    };
    let project = with(project, settings)?;
    // Mouse events come only while it moves: the middle one between two clicks is on the way.
    let (first, second) = (after(&clicks, 0, 0.0)?, after(&clicks, 1, 0.0)?);
    let moves: Vec<_> = tracks
        .values()
        .flat_map(|t| &t.moves)
        .map(|m| m.at.as_secs_f64())
        .filter(|&at| at > first && at < second)
        .collect();
    let on_the_way = *moves
        .get(moves.len() / 2)
        .ok_or("no moves between the clicks")?;
    crate::common::check("cursor/motion", &project, tracks, on_the_way)
}

#[test]
fn golden_cursor_squeezed_by_a_click() -> TestResult {
    let (project, tracks, clicks) = fixture()?;
    let settings = CursorSettings {
        bounce: 5.0,
        ..CursorSettings::default()
    };
    let project = with(project, settings)?;
    // Half of the 350 ms bounce: the deepest squeeze, 72 %.
    crate::common::check("cursor/bounce", &project, tracks, after(&clicks, 2, 0.175)?)
}

/// 0.4 s after a press its effect is 225 ms into its 600 ms (it starts half a bounce later).
#[test]
fn golden_ripple_of_a_click() -> TestResult {
    let (project, tracks, clicks) = fixture()?;
    let project = with(project, clicking(ClickKind::Ripple))?;
    crate::common::check(
        "cursor/click_ripple",
        &project,
        tracks,
        after(&clicks, 0, 0.4)?,
    )
}

#[test]
fn golden_echo_of_a_click() -> TestResult {
    let (project, tracks, clicks) = fixture()?;
    let project = with(project, clicking(ClickKind::Echo))?;
    crate::common::check(
        "cursor/click_echo",
        &project,
        tracks,
        after(&clicks, 1, 0.4)?,
    )
}

#[test]
fn golden_flash_of_a_click() -> TestResult {
    let (project, tracks, clicks) = fixture()?;
    let project = with(project, clicking(ClickKind::Flash))?;
    crate::common::check(
        "cursor/click_flash",
        &project,
        tracks,
        after(&clicks, 2, 0.4)?,
    )
}

/// Ripple for all, but this click is a red spotlight.
#[test]
fn golden_one_click_changed_to_a_spotlight() -> TestResult {
    let (project, tracks, clicks) = fixture()?;
    let media = project
        .clips
        .first()
        .ok_or("fixture without clips")?
        .media
        .path
        .clone();
    let change = ClickChange {
        media,
        at: *clicks.get(3).ok_or("fixture without that click")?,
        kind: Some(ClickKind::Spotlight),
        color: Some(Rgba([0xef, 0x44, 0x44, 0xff])),
        scale: None,
        opacity: None,
        duration: None,
    };
    let project = with_clicks(project, clicking(ClickKind::Ripple), vec![change])?;
    crate::common::check(
        "cursor/click_changed",
        &project,
        tracks,
        after(&clicks, 3, 0.4)?,
    )
}
