use dc_core::project::{Media, MediaKind, SpeedRange};
use dc_core::{
    Clip, OutputTime, PixelSize, Point, Project, Range, Rect, SourceTime, TimeDelta, TimeMap,
};
use dc_render::{Camera, Scene};
use serde_json::json;

use super::CameraTrack;
use crate::{Mode, Preset, Zoom, ZoomState};

const HD: PixelSize = PixelSize {
    width: 1920,
    height: 1080,
};

fn secs(s: f64) -> SourceTime {
    SourceTime::from_secs_f64(s)
}

fn at(s: f64) -> OutputTime {
    OutputTime::from_secs_f64(s)
}

fn project(length: f64) -> Project {
    let mut p = Project::default();
    let id = p.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(length);
    p.clips.push(Clip {
        id,
        media: Media {
            path: "media/recording-1/video.mp4".to_owned(),
            kind: MediaKind::Recording,
            duration,
            size: HD,
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration).unwrap(),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
    });
    p
}

fn zoom(state: &mut ZoomState, from: f64, to: f64, scale: f64, focus: (f64, f64)) {
    let id = state.ids.allocate();
    state.zooms.push(Zoom {
        id,
        media: "media/recording-1/video.mp4".to_owned(),
        range: Range::new(secs(from), secs(to)).unwrap(),
        scale,
        focus: Point {
            x: focus.0,
            y: focus.1,
        },
        mode: Mode::Fixed,
    });
}

fn one_zoom(from: f64, to: f64) -> ZoomState {
    let mut state = ZoomState::default();
    zoom(&mut state, from, to, 2.0, (0.3, 0.4));
    state
}

fn track(p: &Project, state: &ZoomState) -> CameraTrack {
    CameraTrack::new(state, &TimeMap::build(p), HD)
}

fn rounded(camera: Camera) -> serde_json::Value {
    let round = |v: f32, digits: f64| (f64::from(v) * digits).round() / digits;
    json!({
        "scale": round(camera.scale, 1e4),
        "x": round(camera.x, 1e2),
        "y": round(camera.y, 1e2),
    })
}

#[test]
fn without_zooms_the_camera_rests() {
    let p = project(10.0);
    let track = track(&p, &ZoomState::default());
    assert!(track.runs.is_empty());
    for ms in (-500..11_000).step_by(100) {
        assert_eq!(
            track.camera(OutputTime::from_micros(ms * 1000)),
            Camera::IDENTITY
        );
    }
}

#[test]
fn fixed_zoom_in_hold_and_out() {
    let p = project(10.0);
    let track = track(&p, &one_zoom(2.0, 5.0));
    let moments = [
        0.0, 1.6, 1.7, 1.75, 1.8, 1.9, 2.0, 2.3, 2.6, 3.5, 4.6, 4.75, 4.9, 5.1, 5.4, 5.8, 6.5,
    ];
    let cameras: Vec<_> = moments
        .iter()
        .map(|&t| json!({ "t": t, "camera": rounded(track.camera(at(t))) }))
        .collect();
    insta::assert_json_snapshot!(cameras);
}

#[test]
fn held_zoom_puts_the_focus_in_the_centre() {
    let p = project(10.0);
    let camera = track(&p, &one_zoom(2.0, 5.0)).camera(at(3.5));
    assert!((camera.scale - 2.0).abs() < 1e-4, "{camera:?}");
    let [x, y] = camera.apply([0.3 * 1920.0, 0.4 * 1080.0]);
    assert!(
        (x - 960.0).abs() < 0.5 && (y - 540.0).abs() < 0.5,
        "{x} {y}"
    );
}

#[test]
fn focus_at_the_edge_keeps_the_frame_covered() {
    let p = project(10.0);
    let mut state = ZoomState::default();
    zoom(&mut state, 2.0, 5.0, 2.0, (1.0, 0.0));
    let track = track(&p, &state);
    for ms in (0..10_000).step_by(20) {
        let camera = track.camera(OutputTime::from_micros(ms * 1000));
        let [left, top] = camera.apply([0.0, 0.0]);
        let [right, bottom] = camera.apply([1920.0, 1080.0]);
        assert!(left <= 1e-3 && top <= 1e-3, "{ms} ms: {camera:?}");
        assert!(
            right >= 1920.0 - 1e-2 && bottom >= 1080.0 - 1e-2,
            "{ms} ms: {camera:?}"
        );
    }
    let held = track.camera(at(3.5));
    assert!(
        (held.x + 1920.0).abs() < 0.5 && held.y.abs() < 0.5,
        "{held:?}"
    );
}

#[test]
fn cut_inside_a_zoom_shortens_it_on_the_output() {
    let mut cut = project(10.0);
    cut.clips[0].cuts = vec![Range::new(secs(3.0), secs(4.0)).unwrap()];
    let shortened = track(&cut, &one_zoom(2.0, 5.0));
    let plain = track(&project(10.0), &one_zoom(2.0, 4.0));
    for ms in (0..7_000).step_by(50) {
        let t = OutputTime::from_micros(ms * 1000);
        assert_eq!(shortened.camera(t), plain.camera(t), "{ms} ms");
    }
    cut.clips[0].cuts = vec![Range::new(secs(1.0), secs(6.0)).unwrap()];
    assert!(track(&cut, &one_zoom(2.0, 5.0)).runs.is_empty());
}

#[test]
fn transitions_keep_their_length_on_a_fast_part() {
    let mut fast = project(14.0);
    fast.clips[0].speeds = vec![SpeedRange {
        range: Range::new(secs(0.0), secs(10.0)).unwrap(),
        factor: 2.0,
    }];
    let doubled = track(&fast, &one_zoom(4.0, 10.0));
    let plain = track(&project(9.0), &one_zoom(2.0, 5.0));
    for ms in (0..7_000).step_by(50) {
        let t = OutputTime::from_micros(ms * 1000);
        assert_eq!(doubled.camera(t), plain.camera(t), "{ms} ms");
    }
}

#[test]
fn zoom_out_is_over_before_the_end_of_the_video() {
    for preset in Preset::ALL {
        let p = project(10.0);
        let mut state = one_zoom(7.0, 10.0);
        (state.settings.zoom_in, state.settings.zoom_out) = preset.durations();
        let last = track(&p, &state).camera(OutputTime::from_micros(9_999_999));
        assert!(last.scale < 1.01, "{preset:?}: {last:?}");
    }
}

#[test]
fn zoom_at_the_start_of_the_video_is_in_from_the_first_frame() {
    let p = project(10.0);
    let first = track(&p, &one_zoom(0.0, 3.0)).camera(OutputTime::ZERO);
    assert!((first.scale - 2.0).abs() < 1e-4, "{first:?}");
}

#[test]
fn without_smoothness_the_camera_is_the_curve() {
    let p = project(10.0);
    let mut state = one_zoom(2.0, 5.0);
    state.settings.smoothness = 0.0;
    let sharp = track(&p, &state);
    assert!((sharp.camera(at(2.0)).scale - 2.0).abs() < 1e-6);
    assert_eq!(sharp.camera(at(4.95)), Camera::IDENTITY);
    let smooth = track(&p, &one_zoom(2.0, 5.0));
    assert!(smooth.camera(at(4.95)).scale > 1.05);
}

#[test]
fn evaluate_sets_the_camera_of_the_scene() {
    let p = project(10.0);
    let track = track(&p, &one_zoom(2.0, 5.0));
    let mut scene = Scene::new(HD);
    track.evaluate(at(3.0), &mut scene);
    assert_eq!(scene.camera, track.camera(at(3.0)));
    assert_eq!(scene.layers.len(), 0);
}
