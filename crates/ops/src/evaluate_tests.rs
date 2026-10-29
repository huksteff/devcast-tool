use dc_core::project::{Media, MediaKind};
use dc_core::{Clip, OutputTime, PixelSize, Project, Range, Rect, SourceTime, TimeDelta, TimeMap};
use dc_render::Scene;
use serde_json::json;

use crate::{EvaluateError, Evaluator};

fn clip(project: &mut Project, path: &str, secs: f64, width: u32, height: u32) {
    let id = project.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(secs);
    project.clips.push(Clip {
        id,
        media: Media {
            path: path.to_owned(),
            kind: MediaKind::Recording,
            duration,
            size: PixelSize { width, height },
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration).unwrap(),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
    });
}

/// A 1080p recording with a cut at 1–2 s, then a 4:3 one.
fn two_clips() -> Project {
    let mut p = Project::default();
    clip(&mut p, "media/recording-1/video.mp4", 4.0, 1920, 1080);
    clip(&mut p, "media/импорт 1.mp4", 2.0, 1440, 1080);
    p.clips[0].cuts = vec![
        Range::new(
            SourceTime::from_secs_f64(1.0),
            SourceTime::from_secs_f64(2.0),
        )
        .unwrap(),
    ];
    p
}

#[test]
fn project_without_clips_has_no_scene() {
    let error = Evaluator::new(&Project::default()).unwrap_err();
    assert_eq!(error, EvaluateError::Empty);
    assert!(error.to_string().contains("devcast clip add"), "{error}");
}

#[test]
fn every_scene_has_the_canvas_size_and_is_black_outside_the_video() {
    let p = two_clips();
    let evaluator = Evaluator::new(&p).unwrap();
    let canvas = evaluator.canvas();
    assert_eq!(evaluator.duration(), TimeMap::build(&p).duration());
    assert_eq!(evaluator.map(), &TimeMap::build(&p));
    let end = OutputTime::ZERO + evaluator.duration();
    assert_eq!(evaluator.evaluate(end), Scene::new(canvas));
    assert_eq!(
        evaluator.evaluate(OutputTime::from_micros(-1)),
        Scene::new(canvas)
    );
    for ms in (0..5_000).step_by(250) {
        assert_eq!(
            evaluator.evaluate(OutputTime::from_micros(ms * 1000)).size,
            canvas
        );
    }
}

#[test]
fn project_scenes_at_the_cut_and_the_join() {
    let p = two_clips();
    let evaluator = Evaluator::new(&p).unwrap();
    let scenes: Vec<_> = [0.0, 0.999_999, 1.0, 2.999_999, 3.0, 4.999_999, 5.0]
        .iter()
        .map(|&t| json!({ "t": t, "scene": evaluator.evaluate(OutputTime::from_secs_f64(t)) }))
        .collect();
    insta::assert_json_snapshot!(scenes);
}

#[test]
fn broken_effect_state_is_reported_once_before_any_frame() {
    let mut p = two_clips();
    p.effects
        .insert("crop".to_owned(), json!({"settings": {"aspect": "9:16"}}));
    let error = Evaluator::new(&p).unwrap_err();
    assert!(matches!(error, EvaluateError::State(_)), "{error}");
    assert!(error.to_string().contains("«crop»"), "{error}");
}

#[test]
fn clips_of_other_proportions_are_fitted_into_the_aspect_frame() {
    let mut p = two_clips();
    p.effects
        .insert("crop".to_owned(), json!({"settings": {"aspect": "4:3"}}));
    let evaluator = Evaluator::new(&p).unwrap();
    let scenes: Vec<_> = [0.5, 3.5]
        .iter()
        .map(|&t| json!({ "t": t, "scene": evaluator.evaluate(OutputTime::from_secs_f64(t)) }))
        .collect();
    insta::assert_json_snapshot!(scenes);
}

#[test]
fn zoom_camera_moves_the_scene_over_a_cut() {
    let mut p = two_clips();
    p.effects.insert(
        "zoom".to_owned(),
        json!({"zooms": [{
            "id": "z1", "media": "media/recording-1/video.mp4", "range": {"start": 500000, "end": 3000000},
            "scale": 2.0, "focus": {"x": 0.25, "y": 0.75}, "mode": "fixed"
        }]}),
    );
    let evaluator = Evaluator::new(&p).unwrap();
    let scenes: Vec<_> = [0.0, 1.0, 1.9, 2.5, 4.0]
        .iter()
        .map(|&t| json!({ "t": t, "scene": evaluator.evaluate(OutputTime::from_secs_f64(t)) }))
        .collect();
    insta::assert_json_snapshot!(scenes);
}
