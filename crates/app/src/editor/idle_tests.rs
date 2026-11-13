//! The editor at rest asks for no frames (decision 002, task 7.34): once the preview, the waves
//! and the presses of the fixture recording came, a still pointer over the timeline, over the
//! preview or outside the window schedules no repaint, and no thread wakes the window.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

use dc_render::{AdapterChoice, Gpu};
use eframe::egui::{self, mutex::RwLock};
use eframe::egui_wgpu::{Renderer, RendererOptions};

use super::e2e_tests::{H, copy, editor};
use super::export_tests::stand_in;
use super::presses::recordings;
use super::window::frame;
use crate::app::Shared;
use crate::gpu::PreviewGpu;
use crate::settings::Store;

/// How long the window is watched at rest.
const REST: Duration = Duration::from_secs(1);
/// Longest wait for a thread of the editor to answer.
const ANSWER: Duration = Duration::from_secs(30);

/// The app's window with the editor of a copy of the fixture, its preview on WARP; every repaint
/// request of its context goes to the returned channel.
fn opened(temp: &tempfile::TempDir) -> (H, Receiver<egui::RequestRepaintInfo>) {
    let root = copy(&temp.path().join("Записи"), "Запись 1");
    let store = Store::load(&temp.path().join("settings.json")).unwrap();
    let mut shared = Shared::new(egui::Context::default(), store);
    shared.exports = Some(stand_in().0);
    let gpu = Gpu::new(AdapterChoice::Software).unwrap();
    let format = eframe::wgpu::TextureFormat::Bgra8Unorm;
    let renderer = Renderer::new(&gpu.device, format, RendererOptions::default());
    shared.gpu = Some(PreviewGpu {
        gpu: Arc::new(gpu),
        renderer: Arc::new(RwLock::new(renderer)),
    });
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(1280.0, 1000.0))
        .build_ui_state(|ui, shared: &mut Shared| frame(ui, shared), shared);
    let (asked, requests) = channel();
    harness.ctx.set_repaint_observer(move |info| {
        let _ = asked.send(info);
    });
    let ctx = harness.ctx.clone();
    harness.state_mut().ctx = ctx;
    assert!(harness.state_mut().open_project(root.root(), true));
    (harness, requests)
}

/// The preview shows a frame, the sound rows have their waves and the recordings their presses.
fn loaded(harness: &H) -> bool {
    let editor = editor(harness);
    let project = editor.doc.project();
    editor.preview.last().is_some()
        && editor
            .tracks
            .sources()
            .all(|s| editor.waves.get(s).is_some())
        && recordings(project)
            .iter()
            .all(|m| editor.presses.get(m).is_some())
}

/// Runs frames until everything came, each time a thread wakes the window.
fn settle(harness: &mut H, requests: &Receiver<egui::RequestRepaintInfo>) {
    loop {
        // Background readers can wake several frames during loading; only assert idle
        // settling after all of them have delivered their data.
        harness.step();
        if loaded(harness) {
            break;
        }
        requests.recv_timeout(ANSWER).unwrap();
    }
    harness.run();
    while requests.try_recv().is_ok() {}
}

/// Nothing is scheduled after the last frame and nothing asks for one while the window rests.
#[track_caller]
fn rests(harness: &H, requests: &Receiver<egui::RequestRepaintInfo>, place: &str) {
    let delay = harness.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    assert_eq!(delay, Duration::MAX, "{place}: a repaint is scheduled");
    match requests.recv_timeout(REST) {
        Err(RecvTimeoutError::Timeout) => {}
        other => panic!("{place}: a frame asked for at rest: {other:?}"),
    }
}

#[test]
fn the_editor_at_rest_asks_for_no_frames() {
    let temp = tempfile::tempdir().unwrap();
    let (mut harness, requests) = opened(&temp);
    harness.hover_at(egui::pos2(640.0, 850.0));
    settle(&mut harness, &requests);
    assert!(editor(&harness).tracks.sources().next().is_some(), "waves");
    rests(&harness, &requests, "the timeline");
    harness.hover_at(egui::pos2(500.0, 300.0));
    settle(&mut harness, &requests);
    rests(&harness, &requests, "the preview");
    harness.remove_cursor();
    settle(&mut harness, &requests);
    rests(&harness, &requests, "outside");
}
