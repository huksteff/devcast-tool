//! Task 5.16: the output frame of every aspect, a crop and clips of other proportions fitted
//! with black fields.

use std::error::Error;

use dc_core::{CursorTracks, Project, Rect};
use dc_fx_crop::{Aspect, Crop, CropSettings, CropState};

use crate::common::{TestResult, clip, size};

fn with_aspect(mut project: Project, aspect: Aspect) -> Result<Project, Box<dyn Error>> {
    let state = CropState {
        settings: CropSettings { aspect },
    };
    project.set_state::<Crop>(&state)?;
    Ok(project)
}

fn check(name: &str, project: &Project, at: f64) -> TestResult {
    crate::common::check(&format!("crop/{name}"), project, CursorTracks::new(), at)
}

#[test]
fn golden_4x3_clip_in_a_16x9_frame() -> TestResult {
    let mut project = Project::default();
    clip(&mut project, "media/a.mp4", size(160, 120), Rect::FULL)?;
    check("aspect_16x9", &with_aspect(project, Aspect::Wide16x9)?, 0.5)
}

#[test]
fn golden_free_frame_is_the_crop() -> TestResult {
    let mut project = Project::default();
    let crop = Rect::new(0.25, 0.1, 0.5, 0.6)?;
    clip(&mut project, "media/a.mp4", size(320, 180), crop)?;
    check("free", &with_aspect(project, Aspect::Free)?, 0.5)
}

#[test]
fn golden_second_clip_of_other_proportions_gets_fields() -> TestResult {
    let mut project = Project::default();
    clip(&mut project, "media/a.mp4", size(320, 180), Rect::FULL)?;
    let top_half = Rect::new(0.0, 0.0, 1.0, 0.5)?;
    clip(&mut project, "media/b.mp4", size(160, 120), top_half)?;
    check("mixed_clips", &with_aspect(project, Aspect::Source)?, 2.5)
}
