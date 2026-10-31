//! Recording through the HUD end to end: «Запись» → recording → «Остановить запись» →
//! «Подготовка записи» → the editor with the new project. It records monitor
//! `DEVCAST_TEST_MONITOR` for real, so it runs only at night with the live tests.

use std::time::{Duration, Instant};

use dc_capture::{RecordingConfig, Source};
use dc_ui_kit::strings::hud;
use eframe::egui;
use egui_kittest::kittest::Queryable;

use crate::app::Shared;
use crate::session::Session;
use crate::settings::{Countdown, Store};

/// Runs frames until `done` or `limit` passes; the recorder works on its own thread.
fn run_until(
    harness: &mut egui_kittest::Harness<'_, Shared>,
    limit: Duration,
    done: impl Fn(&Shared) -> bool,
) {
    let start = Instant::now();
    while !done(harness.state()) {
        assert!(start.elapsed() < limit, "не дождались за {limit:?}");
        harness.step();
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[ignore = "records the screen"]
fn recording_through_the_hud_opens_the_project() {
    let monitor = std::env::var("DEVCAST_TEST_MONITOR").map_or(2, |m| m.parse().unwrap());
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::load(&dir.path().join("settings.json")).unwrap();
    store
        .update(|s| {
            s.recordings_dir = dir.path().join("Записи DevCast");
            s.countdown = Countdown::Off;
            s.recording = RecordingConfig::new(Source::Monitor(monitor));
        })
        .unwrap();
    let ctx = egui::Context::default();
    let shared = Shared::new(ctx, store);
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(560.0, 48.0))
        .build_ui_state(
            |ui, shared: &mut Shared| {
                shared.tick();
                crate::hud::bar_ui(ui, shared);
            },
            shared,
        );
    harness.run_steps(2);
    harness.get_by_label(hud::RECORD).click();
    run_until(&mut harness, Duration::from_secs(20), |s| {
        matches!(s.session, Session::Live(_))
    });
    std::thread::sleep(Duration::from_secs(3));
    harness.get_by_label(hud::STOP).click();
    run_until(&mut harness, Duration::from_secs(60), |s| {
        s.editor.is_some()
    });
    let editor = harness.state().editor.as_ref().unwrap();
    assert_eq!(editor.doc.project().clips.len(), 1);
    let length = editor.tracks.duration.micros();
    assert!(length >= 2_500_000, "{length}");
    let root = editor.doc.dir().root();
    assert!(root.starts_with(dir.path().join("Записи DevCast")));
    let project = dc_project::ProjectDir::open(root).unwrap().load().unwrap();
    assert_eq!(project.clips.len(), 1);
}
