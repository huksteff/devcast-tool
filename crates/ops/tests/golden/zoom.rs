//! Task 5.17: the zoom camera held, halfway in and at the edge of the picture.

use std::error::Error;

use dc_core::{CursorTracks, Project, Range, Rect, SourceTime, TimeDelta};

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
