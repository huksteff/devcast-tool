//! Task 5.17: the zoom camera held, halfway in and at the edge of the picture. Task 5.18: a
//! follow zoom at the cursor.

use std::error::Error;

use dc_core::{
    CursorPoint, CursorTrack, CursorTracks, Point, Project, Range, Rect, SourceTime, TimeDelta,
};

use crate::common::{TestResult, clip, size};

/// A 4 s 320×180 clip with one zoom over 0.5–2.5 s.
fn zoomed(scale: f64, x: f64, y: f64) -> Result<Project, Box<dyn Error>> {
    let mut project = Project::default();
    clip(&mut project, "media/a.mp4", size(320, 180), Rect::FULL)?;
    let length = TimeDelta::from_secs_f64(4.0);
    for c in &mut project.clips {
        c.media.duration = length;
        c.trim = Range::new(SourceTime::ZERO, SourceTime::ZERO + length)?;
    }
    project.effects.insert(
        "zoom".to_owned(),
        serde_json::json!({"zooms": [{
            "id": "z1", "media": "media/a.mp4", "range": {"start": 500_000, "end": 2_500_000},
            "scale": scale, "focus": {"x": x, "y": y}, "mode": "fixed"
        }]}),
    );
    Ok(project)
}

fn check(name: &str, project: &Project, at: f64) -> TestResult {
    crate::common::check(&format!("zoom/{name}"), project, CursorTracks::new(), at)
}

#[test]
fn golden_zoom_held_puts_the_focus_in_the_centre() -> TestResult {
    check("held", &zoomed(2.0, 0.25, 0.75)?, 1.5)
}

#[test]
fn golden_zoom_halfway_in() -> TestResult {
    check("entering", &zoomed(2.0, 0.25, 0.75)?, 0.45)
}

#[test]
fn golden_zoom_at_the_corner_keeps_the_picture_covered() -> TestResult {
    check("corner", &zoomed(3.0, 1.0, 0.0)?, 1.5)
}

/// The pointer at the left of the picture until 1.2 s, then at the bottom right.
fn wandering() -> CursorTracks {
    let point = |secs: f64, x: f64, y: f64| CursorPoint {
        at: SourceTime::from_secs_f64(secs),
        pos: Point { x, y },
    };
    let track = CursorTrack {
        moves: vec![point(0.0, 0.1, 0.5), point(1.2, 0.85, 0.8)],
        ..CursorTrack::default()
    };
    CursorTracks::from([("media/a.mp4".to_owned(), track)])
}

/// Task 5.18: a follow zoom looks where the cursor is, not at its focus.
fn follow(at: f64, name: &str) -> TestResult {
    let mut project = zoomed(2.0, 0.5, 0.5)?;
    if let Some(zoom) = project.effects.get_mut("zoom") {
        zoom["zooms"][0]["mode"] = "follow".into();
    }
    crate::common::check(&format!("zoom/{name}"), &project, wandering(), at)
}

#[test]
fn golden_follow_zoom_starts_at_the_cursor() -> TestResult {
    follow(1.0, "follow_left")
}

#[test]
fn golden_follow_zoom_moves_to_the_cursor() -> TestResult {
    follow(2.0, "follow_moved")
}
