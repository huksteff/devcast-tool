use std::sync::{Arc, Mutex};

use eframe::egui;

use super::{Shared, lock, show_windows};
use crate::hud::popover::{self, Kind, Open};
use crate::settings::Store;
use crate::settings_window::ID as SETTINGS_ID;

const KINDS: [Kind; 7] = [
    Kind::Source,
    Kind::Sound,
    Kind::Timing,
    Kind::More,
    Kind::Cancel,
    Kind::Quit,
    Kind::Failed,
];

/// A context of an integration with real windows, as eframe's.
fn context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    ctx
}

fn shared(ctx: &egui::Context, dir: &tempfile::TempDir) -> Arc<Mutex<Shared>> {
    let store = Store::load(&dir.path().join("settings.json")).unwrap();
    Arc::new(Mutex::new(Shared::new(ctx.clone(), store)))
}

fn bar() -> Option<egui::Rect> {
    Some(egui::Rect::from_min_size(
        egui::pos2(800.0, 900.0),
        egui::vec2(560.0, 48.0),
    ))
}

fn input(viewport: egui::ViewportId) -> egui::RawInput {
    let mut input = egui::RawInput {
        viewport_id: viewport,
        // As a real GPU gives (8192 and more); the default 2048 is smaller than a monitor's
        // still frame of the area selection.
        max_texture_side: Some(8192),
        ..Default::default()
    };
    input
        .viewports
        .insert(viewport, egui::ViewportInfo::default());
    input
}

/// A pass of `viewport`; the main window's shows the other windows.
fn pass(
    ctx: &egui::Context,
    shared: &Arc<Mutex<Shared>>,
    viewport: egui::ViewportId,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(input(viewport), |ui| {
        if viewport == egui::ViewportId::ROOT {
            show_windows(ui.ctx(), shared, bar());
        }
    });
    output.textures_delta.clear();
    output
}

/// Every window besides the main one is one of ours: with its title and its ui. A window with
/// the default builder is the empty «egui window».
fn others(output: &egui::FullOutput) -> Vec<(egui::ViewportId, &egui::ViewportOutput)> {
    let others: Vec<_> = output
        .viewport_output
        .iter()
        .filter(|(id, _)| **id != egui::ViewportId::ROOT)
        .map(|(id, out)| (*id, out))
        .collect();
    for (id, out) in &others {
        assert!(out.builder.title.is_some(), "{id:?} has no title");
        assert!(out.viewport_ui_cb.is_some(), "{id:?} has no ui");
    }
    others
}

#[test]
fn every_popover_opens_its_own_window() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context();
    let shared = shared(&ctx, &dir);
    for kind in KINDS {
        lock(&shared).popover = Some(Open::new(kind, 0.0));
        let output = pass(&ctx, &shared, egui::ViewportId::ROOT);
        let others = others(&output);
        assert_eq!(others.len(), 1, "{kind:?}");
        let (id, out) = others[0];
        assert_eq!(id, egui::ViewportId::from_hash_of(popover::ID));
        let builder = &out.builder;
        assert_eq!(builder.title.as_deref(), Some(kind.title()));
        assert_eq!(builder.decorations, Some(false), "{kind:?}");
        assert_eq!(builder.resizable, Some(false), "{kind:?}");
        assert_eq!(builder.taskbar, Some(false), "{kind:?}");
        assert!(builder.inner_size.is_some() && builder.position.is_some());
    }
}

/// Threads wake only the main window: a repaint asked of the settings while they are closed
/// opened an empty default window, and closing it crashed on Vulkan.
#[test]
fn levels_and_previews_open_no_empty_window() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context();
    let shared = shared(&ctx, &dir);
    lock(&shared).popover = Some(Open::new(Kind::Sound, 0.0));
    others(&pass(&ctx, &shared, egui::ViewportId::ROOT));
    let popover_id = egui::ViewportId::from_hash_of(popover::ID);
    lock(&shared).popover = None;
    let mut output = ctx.run_ui(input(popover_id), |ui| {
        // A level arriving from the meter thread in the middle of the popover's pass.
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    });
    output.textures_delta.clear();
    others(&output);
    let after = pass(&ctx, &shared, egui::ViewportId::ROOT);
    assert!(others(&after).is_empty());
    assert!(
        !after
            .viewport_output
            .contains_key(&egui::ViewportId::from_hash_of(SETTINGS_ID))
    );
}

/// The picker's window ends at the end of the frame that closes it. The area selection opened in
/// that same frame lost the keyboard focus to that end (Esc and Enter did nothing until a click)
/// and caught the picker in its still frame; it opens a frame later.
#[test]
fn area_selection_opens_after_the_picker_window_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context();
    let shared = shared(&ctx, &dir);
    lock(&shared).area_asked = Some(crate::area::AreaAsk::PickerClosing);
    pass(&ctx, &shared, egui::ViewportId::ROOT);
    assert_eq!(
        lock(&shared).area_asked,
        Some(crate::area::AreaAsk::PickerGone)
    );
    assert!(
        lock(&shared).area.is_none(),
        "not in the picker's last frame"
    );
    pass(&ctx, &shared, egui::ViewportId::ROOT);
    let shared = lock(&shared);
    assert!(shared.area_asked.is_none());
    assert!(shared.area.is_some());
}
