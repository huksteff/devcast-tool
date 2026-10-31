//! Playback in the editor window (7.5): Space, the transport buttons, frame steps and jumps, the
//! stop at the end. The harness has no sound thread, so the wall clock moves the playhead; the
//! test sets the moment of each frame.

use std::time::{Duration, Instant};

use dc_fx_annotations::{Annotations, AnnotationsState, Key as KeyFrame};
use dc_ui_kit::strings::editor as text;
use egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable;

use super::tests::{State, harness, project};
use super::*;
use dc_core::{OutputTime, Rect, TimeDelta};

fn at(secs: f64) -> OutputTime {
    OutputTime::from_secs_f64(secs)
}

/// A press and release of `key`: egui marks a press of a key still down as a repeat.
fn press(harness: &mut egui_kittest::Harness<'_, State>, key: Key) {
    for pressed in [true, false] {
        harness.event(egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        harness.run();
    }
}

/// Runs a frame `secs` after `t0`.
fn frame_at(harness: &mut egui_kittest::Harness<'_, State>, t0: Instant, secs: f64) {
    harness.state_mut().now = Some(t0 + Duration::from_secs_f64(secs));
    harness.run();
}

fn playhead(harness: &egui_kittest::Harness<'_, State>) -> OutputTime {
    harness.state().editor.view.playhead
}

fn playing(harness: &egui_kittest::Harness<'_, State>) -> bool {
    harness.state().editor.playback.is_playing()
}

fn started(t0: Instant) -> egui_kittest::Harness<'static, State> {
    let mut harness = harness(SIZE);
    harness.state_mut().now = Some(t0);
    harness.run();
    harness
}

#[test]
fn space_plays_and_pauses_and_the_button_follows() {
    let t0 = Instant::now();
    let mut harness = started(t0);
    harness.get_by_label(text::PLAY);
    press(&mut harness, Key::Space);
    assert!(playing(&harness));
    harness.get_by_label(text::PAUSE);
    frame_at(&mut harness, t0, 2.0);
    assert_eq!(playhead(&harness), at(35.4), "33.4 s + 2 s of the clock");
    harness.snapshot("editor_playing");
    press(&mut harness, Key::Space);
    assert!(!playing(&harness));
    frame_at(&mut harness, t0, 5.0);
    assert_eq!(playhead(&harness), at(35.4), "paused: the playhead stays");
    harness.get_by_label(text::PLAY).click();
    harness.run();
    frame_at(&mut harness, t0, 5.5);
    assert_eq!(
        playhead(&harness),
        at(35.9),
        "the button resumes from the pause"
    );
}

#[test]
fn a_held_space_does_not_toggle_again() {
    let mut harness = started(Instant::now());
    for _ in 0..2 {
        harness.event(egui::Event::Key {
            key: Key::Space,
            physical_key: Some(Key::Space),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        harness.run();
    }
    assert!(
        playing(&harness),
        "the second press of a key still down is a repeat"
    );
}

#[test]
fn playback_stops_at_the_end_and_starts_over_from_there() {
    let t0 = Instant::now();
    let mut harness = started(t0);
    let end = harness.state().editor.end();
    assert_eq!(end, at(105.0));
    harness.state_mut().editor.view.playhead = at(104.5);
    press(&mut harness, Key::Space);
    frame_at(&mut harness, t0, 1.0);
    assert_eq!(playhead(&harness), end);
    assert!(!playing(&harness), "stopped at the end");
    harness.get_by_label(text::PLAY);
    press(&mut harness, Key::Space);
    assert!(playing(&harness));
    assert_eq!(playhead(&harness), OutputTime::ZERO, "from the start");
}

#[test]
fn arrows_step_a_frame_and_pause() {
    let t0 = Instant::now();
    let mut harness = started(t0);
    assert_eq!(
        harness.state().editor.fps,
        30.0,
        "no readable video: 30 fps"
    );
    press(&mut harness, Key::ArrowRight);
    assert_eq!(
        playhead(&harness).micros(),
        33_433_333,
        "frame 1003 of 30 fps"
    );
    press(&mut harness, Key::ArrowLeft);
    press(&mut harness, Key::ArrowLeft);
    assert_eq!(playhead(&harness).micros(), 33_366_667);
    press(&mut harness, Key::Space);
    frame_at(&mut harness, t0, 1.0);
    press(&mut harness, Key::ArrowRight);
    assert!(!playing(&harness), "a step pauses");
    assert_eq!(playhead(&harness).micros(), 34_400_000);
    harness.get_by_label(text::PREV_FRAME).click();
    harness.run();
    assert_eq!(playhead(&harness).micros(), 34_366_667);
}

#[test]
fn brackets_leap_5_s_without_keys_and_playback_goes_on() {
    let t0 = Instant::now();
    let mut harness = started(t0);
    press(&mut harness, Key::CloseBracket);
    assert_eq!(playhead(&harness), at(38.4));
    press(&mut harness, Key::OpenBracket);
    press(&mut harness, Key::OpenBracket);
    assert_eq!(playhead(&harness), at(28.4));
    press(&mut harness, Key::Space);
    frame_at(&mut harness, t0, 1.0);
    harness.get_by_label(text::NEXT_KEY).click();
    harness.run();
    assert!(playing(&harness), "a jump keeps playing");
    assert_eq!(playhead(&harness), at(34.4));
    frame_at(&mut harness, t0, 1.5);
    assert_eq!(playhead(&harness), at(34.9));
}

#[test]
fn keyframes_of_annotations_are_the_jump_targets_at_output_times() {
    let mut project = project();
    let mut state: AnnotationsState = project.state::<Annotations>().unwrap();
    // Text a1 shows source 28..40 of the screen: 20..25 is cut, 30..38 plays at 4×.
    let key = |secs: f64| KeyFrame {
        at: TimeDelta::from_secs_f64(secs),
        rect: Rect::new(0.1, 0.1, 0.3, 0.1).unwrap(),
        rotation: 0.0,
        body: serde_json::Map::new(),
    };
    state.annotations[0].keys = vec![key(1.0), key(6.0), key(20.0)];
    project.set_state::<Annotations>(&state).unwrap();
    let tracks = Tracks::of(&project, &[], Default::default());
    assert_eq!(
        tracks.keys,
        vec![at(24.0), at(26.0)],
        "source 29 → 24, 34 → 25 + 4/4; a key past the end is not shown"
    );
    use super::playback::seek::jump;
    assert_eq!(jump(at(23.0), true, &tracks.keys, at(105.0)), at(24.0));
    assert_eq!(jump(at(30.0), false, &tracks.keys, at(105.0)), at(26.0));
}
