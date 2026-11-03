use std::sync::{Arc, Mutex};
use std::time::Duration;

use dc_core::OutputTime;
use dc_ui_kit::strings::timeline as names;
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::kittest::Queryable;

use super::editing_tests::{by_cli, json, undone};
use super::test_project::project;
use super::timeline_window::{self, ID, frame, on_screen};
use super::{EditorWindow, window};
use crate::app::{Shared, lock, other_window};
use crate::settings::{EditorLayout, Store, WindowPlace};

type Both = Arc<Mutex<Shared>>;
type H = egui_kittest::Harness<'static, Both>;

/// Shared state with the editor of the test project and the settings in a temporary folder;
/// the folders live while the guards do.
fn shared() -> (tempfile::TempDir, tempfile::TempDir, Both) {
    let (project, editor) = super::tests::editor();
    let settings = tempfile::tempdir().unwrap();
    let store = Store::load(&settings.path().join("settings.json")).unwrap();
    let mut shared = Shared::new(egui::Context::default(), store);
    shared.editor = Some(editor);
    (project, settings, Arc::new(Mutex::new(shared)))
}

/// The timeline window over `both`, opened.
fn timeline(both: &Both) -> H {
    lock(both).editor.as_mut().unwrap().detach(true);
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(1280.0, 460.0))
        .build_ui_state(
            |ui, both: &mut Both| other_window(ui, both, frame),
            Arc::clone(both),
        );
    harness.run();
    harness
}

/// The editor window over `both`.
fn editor(both: &Both) -> H {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(super::SIZE)
        .build_ui_state(
            |ui, both: &mut Both| other_window(ui, both, window::frame),
            Arc::clone(both),
        );
    harness.run();
    harness
}

fn press(harness: &mut H, key: Key, modifiers: Modifiers) {
    harness.event(Event::ModifiersChanged(modifiers));
    harness.event(Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    });
    harness.event(Event::ModifiersChanged(Modifiers::NONE));
    harness.run();
}

fn edited(both: &Both, change: impl FnOnce(&mut EditorWindow)) {
    change(lock(both).editor.as_mut().unwrap());
}

#[test]
fn the_window_shows_the_same_tracks() {
    let (_project, _settings, both) = shared();
    let mut harness = timeline(&both);
    harness.get_by_label(&names::annotations(2));
    harness.get_by_label(&names::audio(1));
    harness.get_by_label(names::SYSTEM);
    harness.get_by_label(names::RULER);
    harness.snapshot("timeline_window");
}

#[test]
fn an_edit_in_the_window_is_the_cli_command_and_one_undo_step() {
    let (_project, _settings, both) = shared();
    edited(&both, |e| e.view.playhead = OutputTime::from_secs_f64(30.0));
    let mut harness = timeline(&both);
    press(&mut harness, Key::C, Modifiers::NONE);
    let after = json(lock(&both).editor.as_ref().unwrap().doc.project());
    assert_eq!(after, json(&by_cli("clip split --at 30")));
    press(&mut harness, Key::Z, Modifiers::COMMAND);
    let shared = lock(&both);
    let now = shared.editor.as_ref().unwrap().doc.project();
    assert_eq!(json(now), undone(&project(), now), "Ctrl+Z works here too");
}

#[test]
fn the_playhead_moves_in_both_windows() {
    let (_project, _settings, both) = shared();
    let mut timeline = timeline(&both);
    timeline.get_by_label(names::RULER).click();
    timeline.run();
    let moved = lock(&both).editor.as_ref().unwrap().view.playhead;
    assert_ne!(moved, OutputTime::from_secs_f64(33.4));
    assert!(lock(&both).editor.as_ref().unwrap().detached.wake_editor);

    let mut editor = editor(&both);
    assert!(
        editor.query_by_label(names::RULER).is_none(),
        "the docked timeline hides while the window is open"
    );
    press(&mut editor, Key::ArrowRight, Modifiers::NONE);
    let shared = lock(&both);
    let e = shared.editor.as_ref().unwrap();
    assert!(e.view.playhead > moved, "one frame on");
    assert!(e.detached.wake_timeline);
}

#[test]
fn the_main_window_repaints_the_marked_window() {
    let (_project, _settings, both) = shared();
    edited(&both, |e| {
        e.detach(true);
        e.detached.wake_timeline = true;
    });
    let ctx = lock(&both).ctx.clone();
    ctx.set_embed_viewports(false);
    let pass = || {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            window::show(ui.ctx(), &both);
        });
        output.textures_delta.clear();
        output.viewport_output[&egui::ViewportId::from_hash_of(ID)].repaint_delay
    };
    assert_eq!(pass(), Duration::ZERO);
    assert!(!lock(&both).editor.as_ref().unwrap().detached.wake_timeline);
    edited(&both, |e| e.detach(false));
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        window::show(ui.ctx(), &both);
    });
    output.textures_delta.clear();
    let id = egui::ViewportId::from_hash_of(ID);
    assert!(!output.viewport_output.contains_key(&id), "closed");
}

#[test]
fn open_state_and_place_are_remembered() {
    let (_project, _settings, both) = shared();
    let mut editor = editor(&both);
    editor.get_by_label(names::DETACH).click();
    editor.run();
    assert!(lock(&both).store.get().editor.timeline_window);
    assert!(lock(&both).editor.as_ref().unwrap().detached.open);

    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    let rect = |x, y, w, h| {
        Some(egui::Rect::from_min_size(
            egui::pos2(x, y),
            egui::vec2(w, h),
        ))
    };
    input.viewports.insert(
        egui::ViewportId::ROOT,
        egui::ViewportInfo {
            events: vec![egui::ViewportEvent::Close],
            outer_rect: rect(100.0, 50.0, 1282.0, 492.0),
            inner_rect: rect(101.0, 81.0, 1280.0, 460.0),
            ..Default::default()
        },
    );
    let mut output = ctx.run_ui(input, |ui| frame(ui, &mut lock(&both)));
    output.textures_delta.clear();
    let place = WindowPlace {
        x: 100.0,
        y: 50.0,
        width: 1280.0,
        height: 460.0,
    };
    let layout = lock(&both).store.get().editor;
    assert!(!layout.timeline_window);
    assert_eq!(layout.timeline_place, Some(place));
    assert!(!lock(&both).editor.as_ref().unwrap().detached.open);

    let (_other, mut next) = super::tests::editor();
    let opened = EditorLayout {
        timeline_window: true,
        ..layout
    };
    timeline_window::restore(&mut next, &opened);
    assert!(next.detached.open);
    assert_eq!(next.detached.place, Some(place));
}

#[test]
fn a_place_off_the_monitors_is_forgotten() {
    let monitor = dc_capture::MonitorInfo {
        index: 1,
        name: None,
        bounds: dc_capture::Area::new(0, 0, 1920, 1080).unwrap(),
        primary: true,
        scale: 100,
    };
    let place = |x| {
        Some(WindowPlace {
            x,
            y: 100.0,
            width: 1280.0,
            height: 460.0,
        })
    };
    let monitors = [monitor];
    assert_eq!(on_screen(place(100.0), &monitors, 1.0), place(100.0));
    assert_eq!(on_screen(place(-1800.0), &monitors, 1.0), None);
    assert_eq!(
        on_screen(place(1000.0), &monitors, 2.0),
        None,
        "middle at 3280 px"
    );
}

#[test]
fn the_window_plays_while_the_editor_gets_no_frames() {
    let (_project, _settings, both) = shared();
    let from = OutputTime::from_secs_f64(10.0);
    edited(&both, |e| {
        e.detach(true);
        let started = std::time::Instant::now() - Duration::from_secs(1);
        assert_eq!(e.playback.play(from, started), None);
    });
    let ctx = egui::Context::default();
    let pass = || {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            frame(ui, &mut lock(&both));
        });
        output.textures_delta.clear();
        output.viewport_output[&egui::ViewportId::ROOT].repaint_delay
    };
    let delay = (0..5).map(|_| pass()).next_back().unwrap();
    let shared = lock(&both);
    let e = shared.editor.as_ref().unwrap();
    assert!(
        (11.0..20.0).contains(&e.view.playhead.as_secs_f64()),
        "the window's own frame moves the playhead from 10 s (editor minimized)"
    );
    assert!(
        !e.detached.wake_editor,
        "playing alone does not repaint the other window"
    );
    assert!(
        delay <= Duration::from_millis(100),
        "the window asks for its next frame itself: {delay:?}"
    );
}
