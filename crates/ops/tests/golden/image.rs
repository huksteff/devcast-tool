//! Task 5.34: image annotations over bar videos — pictures fitted into wide and tall boxes, a
//! frame of an animated GIF by output time, turned pictures under the zoom camera with a pop.

use std::error::Error;

use dc_core::{Command, OutputTime, Project, Range, Rect, SourceTime, TimeDelta};
use dc_fx_annotations::{AnnotAdd, AnnotAnim};
use dc_fx_image::{ImageSet, PictureFile};
use serde_json::{Value, json};

use crate::common::{TestResult, check_pictures, clip, fixtures, size};

/// `fixtures/images/<name>` as the scene needs it.
fn picture(name: &str) -> Result<PictureFile, Box<dyn Error>> {
    let path = format!("images/{name}");
    let decoded = dc_media::decode_picture(&fixtures().join(&path))?;
    let durations: Vec<_> = decoded.frames.iter().map(|f| f.duration).collect();
    Ok(PictureFile::new(path, decoded.size, &durations))
}

/// A 2 s 640×360 clip with an image annotation over 0.2–1.8 s for each `(place, picture)`:
/// `place` goes to `annot add`.
fn project(images: &[(Value, &str)]) -> Result<Project, Box<dyn Error>> {
    let mut project = Project::default();
    clip(&mut project, "media/a.mp4", size(640, 360), Rect::FULL)?;
    let length = TimeDelta::from_secs_f64(2.0);
    for c in &mut project.clips {
        c.media.duration = length;
        c.trim = Range::new(SourceTime::ZERO, SourceTime::ZERO + length)?;
    }
    for (i, (place, name)) in images.iter().enumerate() {
        let mut add = json!({"kind": "image", "at": 0.2, "duration": 1.6});
        if let (Some(add), Value::Object(place)) = (add.as_object_mut(), place.clone()) {
            add.extend(place);
        }
        serde_json::from_value::<AnnotAdd>(add)?.apply(&mut project)?;
        let set = ImageSet {
            id: format!("a{}", i + 1).parse()?,
            picture: picture(name)?,
            at: None::<OutputTime>,
        };
        set.apply(&mut project)?;
    }
    Ok(project)
}

/// At output 0.5 s: 0.3 s after the annotations appear, frame 1 of the 200 ms GIF.
fn check(name: &str, project: &Project) -> TestResult {
    check_pictures(&format!("image/{name}"), project, 0.5)
}

#[test]
fn golden_pictures_fit_their_boxes() -> TestResult {
    let project = project(&[
        (json!({"rect": "0.04,0.08,0.5,0.35"}), "sticker.png"),
        (json!({"rect": "0.62,0.08,0.15,0.84"}), "sticker.png"),
        (json!({"rect": "0.04,0.52,0.5,0.4"}), "blink.gif"),
    ])?;
    check("fit", &project)
}

#[test]
fn golden_turned_pictures_under_the_zoom_camera() -> TestResult {
    let mut project = project(&[
        (
            json!({"rect": "0.12,0.3,0.3,0.3", "rotation": 20}),
            "sticker.png",
        ),
        (
            json!({"rect": "0.35,0.45,0.3,0.3", "rotation": -15}),
            "blink.gif",
        ),
    ])?;
    let pop = json!({"id": "a1", "enter": "pop", "enter_duration": 0.6});
    serde_json::from_value::<AnnotAnim>(pop)?.apply(&mut project)?;
    project.effects.insert(
        "zoom".to_owned(),
        json!({"zooms": [{
            "id": "z1", "media": "media/a.mp4", "range": {"start": 0, "end": 2_000_000},
            "scale": 1.6, "focus": {"x": 0.35, "y": 0.5}, "mode": "fixed"
        }]}),
    );
    check("turned", &project)
}
