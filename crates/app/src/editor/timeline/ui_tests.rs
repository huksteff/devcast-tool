//! The timeline in the editor window (7.6, 7.7): scale, scroll, row heights, the navigation bar,
//! the playhead on the ruler. Points come from the widgets' rects; times are checked against the
//! viewport math (`viewport_tests.rs`).

use std::time::{Duration, Instant};

use dc_core::{OutputTime, TimeDelta};
use dc_ui_kit::strings::timeline as text;
use eframe::egui::{
    self, Event, Key, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, Rect,
    TouchPhase, pos2, vec2,
};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::viewport::Lanes;
use crate::editor::SIZE;
use crate::editor::tests::{State, harness};

type H = Harness<'static, State>;

fn started() -> H {
    let mut harness = harness(SIZE);
    harness.run();
    harness
}

fn rect(harness: &H, label: &str) -> Rect {
    harness.get_by_label(label).rect()
}

/// The lanes as the ruler spans them.
fn lanes(harness: &H) -> Lanes {
    let ruler = rect(harness, text::RULER);
    Lanes {
        left: ruler.left(),
        width: ruler.width(),
        duration: TimeDelta::from_secs_f64(105.0),
        fps: 30.0,
    }
}

/// x of output second `secs` on the lanes now.
fn x_of(harness: &H, secs: f64) -> f32 {
    let lanes = lanes(harness);
    let view = &harness.state().editor.view.timeline;
    view.scale(&lanes).x(OutputTime::from_secs_f64(secs))
}

fn secs(t: OutputTime) -> f64 {
    t.as_secs_f64()
}

fn playhead(harness: &H) -> f64 {
    secs(harness.state().editor.view.playhead)
}

fn start(harness: &H) -> f64 {
    let lanes = lanes(harness);
    harness.state().editor.view.timeline.start(&lanes)
}

fn button(harness: &H, pos: Pos2, pressed: bool) {
    harness.event(Event::PointerMoved(pos));
    harness.event(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    });
}

fn move_to(harness: &mut H, pos: Pos2) {
    harness.event(Event::PointerMoved(pos));
    harness.run();
}

fn wheel(harness: &mut H, at: Pos2, delta: egui::Vec2, modifiers: Modifiers) {
    harness.event(Event::PointerMoved(at));
    let event = Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta,
        phase: TouchPhase::Move,
        source: MouseWheelSource::Unknown,
        modifiers,
    };
    harness.event_modifiers(event, modifiers);
    harness.run();
}

/// The middle of the lanes at the height of the clips row.
fn over_clips(harness: &H) -> Pos2 {
    let clips = rect(harness, text::CLIPS);
    let lanes = lanes(harness);
    pos2(lanes.left + lanes.width / 2.0, clips.center().y)
}

fn zoom(harness: &mut H, factor: f64, anchor: f32) {
    let lanes = lanes(harness);
    let view = &mut harness.state_mut().editor.view.timeline;
    view.zoom_by(&lanes, factor, anchor);
    harness.run();
}

#[test]
fn a_click_on_the_ruler_moves_the_playhead() {
    let mut harness = started();
    let y = rect(&harness, text::RULER).center().y;
    let x = x_of(&harness, 80.0);
    button(&harness, pos2(x, y), true);
    button(&harness, pos2(x, y), false);
    harness.run();
    assert!(
        (playhead(&harness) - 80.0).abs() < 0.1,
        "{}",
        playhead(&harness)
    );
    assert_eq!(harness.state().editor.view.timeline.scrub, None);
}

#[test]
fn dragging_the_playhead_scrubs_with_keyframes() {
    let mut harness = started();
    let line = rect(&harness, text::PLAYHEAD);
    let y = line.center().y;
    button(&harness, pos2(line.center().x, y), true);
    harness.run();
    for secs in [40.0, 50.0, 60.0] {
        let x = x_of(&harness, secs);
        move_to(&mut harness, pos2(x, y));
        assert!((playhead(&harness) - secs).abs() < 0.1);
        assert!(
            harness.state().editor.view.timeline.scrub.is_some(),
            "the preview takes keyframes"
        );
    }
    button(&harness, pos2(x_of(&harness, 60.0), y), false);
    harness.run();
    assert_eq!(
        harness.state().editor.view.timeline.scrub,
        None,
        "exact frame again"
    );
    assert!((playhead(&harness) - 60.0).abs() < 0.1);
}

#[test]
fn during_playback_a_click_seeks_and_playback_goes_on() {
    let t0 = Instant::now();
    let mut harness = started();
    harness.state_mut().now = Some(t0);
    harness.key_press(Key::Space);
    harness.run();
    assert!(harness.state().editor.playback.is_playing());
    let at = pos2(x_of(&harness, 80.0), rect(&harness, text::RULER).center().y);
    button(&harness, at, true);
    button(&harness, at, false);
    harness.run();
    assert!(harness.state().editor.playback.is_playing(), "goes on");
    harness.state_mut().now = Some(t0 + Duration::from_secs(1));
    harness.run();
    assert!(
        (playhead(&harness) - 81.0).abs() < 0.1,
        "{}",
        playhead(&harness)
    );
}

#[test]
fn a_drag_during_playback_pauses_until_the_release() {
    let t0 = Instant::now();
    let mut harness = started();
    harness.state_mut().now = Some(t0);
    harness.key_press(Key::Space);
    harness.run();
    let y = rect(&harness, text::RULER).center().y;
    button(&harness, pos2(x_of(&harness, 20.0), y), true);
    harness.run();
    let x = x_of(&harness, 30.0);
    move_to(&mut harness, pos2(x, y));
    assert!(
        !harness.state().editor.playback.is_playing(),
        "paused while dragged"
    );
    harness.state_mut().now = Some(t0 + Duration::from_secs(5));
    harness.run();
    assert!((playhead(&harness) - 30.0).abs() < 0.1);
    button(&harness, pos2(x_of(&harness, 30.0), y), false);
    harness.run();
    assert!(
        harness.state().editor.playback.is_playing(),
        "plays on after the release"
    );
}

#[test]
fn ctrl_wheel_zooms_around_the_pointer() {
    let mut harness = started();
    let at = over_clips(&harness);
    let lanes = lanes(&harness);
    let before = harness.state().editor.view.timeline.scale(&lanes);
    wheel(&mut harness, at, vec2(0.0, 1.0), Modifiers::CTRL);
    let after = harness.state().editor.view.timeline.scale(&lanes);
    assert!(after.px_per_sec > before.px_per_sec * 1.1, "{after:?}");
    let drift = secs(after.time(at.x)) - secs(before.time(at.x));
    assert!(
        drift.abs() < 0.05,
        "the time under the pointer stays: {drift}"
    );
    harness.snapshot("editor_timeline_zoomed");
    wheel(&mut harness, at, vec2(0.0, -20.0), Modifiers::CTRL);
    assert_eq!(start(&harness), 0.0, "zoomed out to the whole project");
}

#[test]
fn the_slider_and_fit_scale_the_time() {
    let mut harness = started();
    let slider = rect(&harness, text::SCALE);
    button(
        &harness,
        pos2(slider.right() - 1.0, slider.center().y),
        true,
    );
    button(
        &harness,
        pos2(slider.right() - 1.0, slider.center().y),
        false,
    );
    harness.run();
    let lanes = lanes(&harness);
    let view = &harness.state().editor.view.timeline;
    assert!(
        (view.px_per_sec(&lanes) - 750.0).abs() < 1.0,
        "the finest scale"
    );
    let x = x_of(&harness, 33.4);
    assert!(
        (x - lanes.left).abs() < lanes.width,
        "the playhead stays in view"
    );
    harness.get_by_label(text::FIT).click();
    harness.run();
    let view = &harness.state().editor.view.timeline;
    assert!((view.visible(&lanes) - 105.0).abs() < 1e-6);
}

#[test]
fn the_wheel_scrolls_time_sideways_and_the_overflowing_rows_down() {
    let mut harness = started();
    zoom(&mut harness, 4.0, 148.0);
    let at = over_clips(&harness);
    let clips = rect(&harness, text::CLIPS).top();
    wheel(&mut harness, at, vec2(0.0, -1.0), Modifiers::NONE);
    assert_eq!(start(&harness), 0.0, "the rows overflow: they scroll");
    assert!(rect(&harness, text::CLIPS).top() < clips - 10.0);
    wheel(&mut harness, at, vec2(-2.0, 0.0), Modifiers::NONE);
    let lanes = lanes(&harness);
    let px = harness.state().editor.view.timeline.px_per_sec(&lanes);
    assert!(
        (start(&harness) - 80.0 / px).abs() < 0.05,
        "{}",
        start(&harness)
    );
}

#[test]
fn shift_wheel_scales_the_rows_and_the_settings_keep_it() {
    let mut harness = started();
    let at = over_clips(&harness);
    let height = rect(&harness, text::CLIPS).height();
    wheel(&mut harness, at, vec2(0.0, 1.0), Modifiers::SHIFT);
    let rows = harness.state().outcome.as_ref().unwrap().layout.rows;
    assert!((rows - 1.1).abs() < 1e-4, "{rows}");
    assert!((rect(&harness, text::CLIPS).height() - height * 1.1).abs() < 0.5);
    assert_eq!(start(&harness), 0.0, "time did not move");
    wheel(&mut harness, at, vec2(0.0, -20.0), Modifiers::SHIFT);
    let rows = harness.state().outcome.as_ref().unwrap().layout.rows;
    assert_eq!(rows, 0.5);
    harness.snapshot("editor_timeline_rows");
}

#[test]
fn the_navigation_thumb_drags_the_view() {
    let mut harness = started();
    zoom(&mut harness, 4.0, 148.0);
    let bar = rect(&harness, text::NAVIGATION);
    let thumb = pos2(bar.left() + bar.width() / 8.0, bar.center().y);
    button(&harness, thumb, true);
    harness.run();
    move_to(&mut harness, thumb + vec2(bar.width() / 4.0, 0.0));
    button(&harness, thumb + vec2(bar.width() / 4.0, 0.0), false);
    harness.run();
    assert!(
        (start(&harness) - 105.0 / 4.0).abs() < 0.05,
        "{}",
        start(&harness)
    );
    let far = pos2(bar.right() - 2.0, bar.center().y);
    button(&harness, far, true);
    harness.run();
    button(&harness, far, false);
    harness.run();
    assert!(
        (start(&harness) - 105.0 * 0.75).abs() < 0.05,
        "a press beside centres there"
    );
}

#[test]
fn dragging_past_the_edge_scrolls_the_view() {
    let mut harness = started();
    zoom(&mut harness, 10.0, 148.0);
    let lanes = lanes(&harness);
    let y = rect(&harness, text::RULER).center().y;
    button(&harness, pos2(x_of(&harness, 5.0), y), true);
    harness.run();
    harness.event(Event::PointerMoved(pos2(lanes.right() + 50.0, y)));
    harness.step();
    harness.step();
    let first = start(&harness);
    for _ in 0..10 {
        harness.step();
    }
    assert!(
        start(&harness) > first,
        "it goes on without moving the mouse"
    );
    let end = secs(
        harness
            .state()
            .editor
            .view
            .timeline
            .scale(&lanes)
            .time(lanes.right()),
    );
    assert!(
        (playhead(&harness) - end).abs() < 0.5,
        "the playhead rides the edge"
    );
    button(&harness, pos2(lanes.right() + 50.0, y), false);
    harness.run();
    assert_eq!(harness.state().editor.view.timeline.scrub, None);
}

#[test]
fn playback_pages_the_view_after_the_playhead() {
    let t0 = Instant::now();
    let mut harness = started();
    zoom(&mut harness, 10.0, 148.0);
    assert!(start(&harness) < 33.4 - 10.5, "the playhead is out of view");
    harness.state_mut().now = Some(t0);
    harness.key_press(Key::Space);
    harness.run();
    harness.state_mut().now = Some(t0 + Duration::from_millis(500));
    harness.run();
    assert!((start(&harness) - 33.9).abs() < 0.1, "{}", start(&harness));
    harness.state_mut().now = Some(t0 + Duration::from_millis(11_500));
    harness.run();
    assert!((start(&harness) - 44.9).abs() < 0.1, "the next page");
}
