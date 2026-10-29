use dc_core::project::{Media, MediaKind, SpeedRange};
use dc_core::{
    Clip, CursorTracks, OutputTime, PixelSize, Point, Project, Range, Rect, SourceTime, TimeDelta,
    TimeMap,
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
    CameraTrack::new(p, state, &TimeMap::build(p), HD, &CursorTracks::new())
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

fn follow_zoom(from: f64, to: f64) -> ZoomState {
    let mut state = one_zoom(from, to);
    state.zooms[0].mode = Mode::Follow;
    state
}

/// The cursor sweeps the frame left to right over 2–5 s, at mid height.
fn sweep(t: f64) -> Option<Point> {
    let x = (0.1 + (t - 2.0) / 3.0 * 0.8).clamp(0.1, 0.9);
    Some(Point { x, y: 0.5 })
}

#[test]
fn follow_zoom_moves_with_the_cursor() {
    let p = project(10.0);
    let track = CameraTrack::simulate(&follow_zoom(2.0, 5.0), &TimeMap::build(&p), HD, sweep);
    let moments = [
        1.7, 1.8, 2.0, 2.4, 2.8, 3.0, 3.2, 3.5, 3.8, 4.0, 4.3, 4.6, 4.8, 5.0, 5.5,
    ];
    let cameras: Vec<_> = moments
        .iter()
        .map(|&t| json!({ "t": t, "camera": rounded(track.camera(at(t))) }))
        .collect();
    insta::assert_json_snapshot!(cameras);
}

#[test]
fn followed_cursor_never_leaves_the_frame() {
    let p = project(10.0);
    let track = CameraTrack::simulate(&follow_zoom(2.0, 5.0), &TimeMap::build(&p), HD, sweep);
    for ms in (1_700..5_000).step_by(10) {
        let t = f64::from(ms) / 1000.0;
        let cursor = sweep(t).unwrap();
        let [x, y] = track
            .camera(at(t))
            .apply([(cursor.x * 1920.0) as f32, (cursor.y * 1080.0) as f32]);
        assert!(
            (0.0..=1920.0).contains(&x) && (0.0..=1080.0).contains(&y),
            "{ms} ms: {x} {y}"
        );
    }
}

#[test]
fn follow_zoom_without_a_cursor_stays_on_its_focus() {
    let p = project(10.0);
    let map = TimeMap::build(&p);
    let blind = CameraTrack::simulate(&follow_zoom(2.0, 5.0), &map, HD, |_| None);
    let fixed = CameraTrack::simulate(&one_zoom(2.0, 5.0), &map, HD, sweep);
    assert_eq!(blind, fixed);
}

/// Two 2× zooms 0.5 s apart: (0.3, 0.4) over 2–4 s and (0.7, 0.6) over 4.5–6.5 s.
fn close_pair(connect: bool) -> ZoomState {
    let mut state = one_zoom(2.0, 4.0);
    zoom(&mut state, 4.5, 6.5, 2.0, (0.7, 0.6));
    state.settings.connect = connect;
    state
}

fn lowest_scale(track: &CameraTrack, from_ms: i64, to_ms: i64) -> f32 {
    (from_ms..to_ms)
        .step_by(10)
        .map(|ms| track.camera(OutputTime::from_micros(ms * 1000)).scale)
        .fold(f32::MAX, f32::min)
}

#[test]
fn connected_zooms_pan_without_zooming_out() {
    let p = project(10.0);
    let connected = track(&p, &close_pair(true));
    assert!(lowest_scale(&connected, 3_000, 5_500) > 1.99);
    let apart = track(&p, &close_pair(false));
    assert!(lowest_scale(&apart, 3_000, 5_500) < 1.6);
    let held = connected.camera(at(6.0));
    let [x, y] = held.apply([0.7 * 1920.0, 0.6 * 1080.0]);
    assert!(
        (x - 960.0).abs() < 1.0 && (y - 540.0).abs() < 1.0,
        "{held:?}"
    );
}

#[test]
fn far_zooms_are_not_connected() {
    let p = project(12.0);
    let mut state = one_zoom(2.0, 4.0);
    zoom(&mut state, 5.4, 7.0, 2.0, (0.7, 0.6));
    assert!(lowest_scale(&track(&p, &state), 3_000, 6_000) < 1.1);
}

#[test]
fn connected_zooms_as_snapshots() {
    let p = project(10.0);
    let track = track(&p, &close_pair(true));
    let moments = [3.6, 3.8, 4.0, 4.1, 4.2, 4.3, 4.5, 4.8, 5.2, 6.4];
    let cameras: Vec<_> = moments
        .iter()
        .map(|&t| json!({ "t": t, "camera": rounded(track.camera(at(t))) }))
        .collect();
    insta::assert_json_snapshot!(cameras);
}

#[test]
fn connected_follow_zooms_keep_following() {
    let p = project(10.0);
    let map = TimeMap::build(&p);
    let mut state = close_pair(true);
    state.zooms[0].mode = Mode::Follow;
    state.zooms[1].mode = Mode::Follow;
    state.zooms[1].focus = Point { x: 0.2, y: 0.5 };
    let right = |_: f64| Some(Point { x: 0.8, y: 0.5 });
    let chained = CameraTrack::simulate(&state, &map, HD, right);
    let [x, _] = chained.camera(at(6.0)).apply([0.75 * 1920.0, 0.5 * 1080.0]);
    assert!((x - 960.0).abs() < 1.0, "{x}");
    state.zooms[1].mode = Mode::Fixed;
    let fixed = CameraTrack::simulate(&state, &map, HD, right);
    let [x, _] = fixed.camera(at(6.0)).apply([0.25 * 1920.0, 0.5 * 1080.0]);
    assert!((x - 960.0).abs() < 1.0, "{x}");
}

#[test]
fn smooth_preset_as_snapshots() {
    let p = project(10.0);
    let mut state = one_zoom(2.0, 5.0);
    (state.settings.zoom_in, state.settings.zoom_out) = Preset::Smooth.durations();
    let track = track(&p, &state);
    let moments = [
        1.6, 1.8, 2.0, 2.4, 2.8, 3.2, 3.6, 4.6, 4.8, 5.0, 5.3, 5.6, 6.0, 6.5, 7.0,
    ];
    let cameras: Vec<_> = moments
        .iter()
        .map(|&t| json!({ "t": t, "camera": rounded(track.camera(at(t))) }))
        .collect();
    insta::assert_json_snapshot!(cameras);
}

#[test]
fn moving_camera_is_blurred_over_its_shutter() {
    let p = project(10.0);
    let track = track(&p, &one_zoom(2.0, 5.0));
    let mut scene = Scene::new(HD);
    track.evaluate(at(1.8), &mut scene);
    let motion = scene.camera_motion.expect("zooming in is blurred");
    let shutter = TimeDelta::from_secs_f64(4.0 / 60.0 * 0.35);
    assert_eq!(motion.from, track.camera(at(1.8) - shutter));
    assert_eq!(motion.samples.get(), 13);
    for t in [0.5, 9.0] {
        let mut still = Scene::new(HD);
        track.evaluate(at(t), &mut still);
        assert_eq!(still.camera_motion, None, "{t} s");
    }
    let mut held = Scene::new(HD);
    track.evaluate(at(3.5), &mut held);
    let from = held.camera_motion.map_or(held.camera, |m| m.from);
    assert!((from.x - held.camera.x).abs() < 0.05, "{from:?}");
    let mut state = one_zoom(2.0, 5.0);
    state.settings.motion_blur = 0.0;
    let sharp = super::CameraTrack::new(&p, &state, &TimeMap::build(&p), HD, &CursorTracks::new());
    let mut scene = Scene::new(HD);
    sharp.evaluate(at(1.8), &mut scene);
    assert_eq!(scene.camera_motion, None);
}
