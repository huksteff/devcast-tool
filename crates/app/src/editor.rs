//! The editor window: opens with the project of a finished recording, the HUD hides meanwhile
//! and comes back when the editor closes (v1). The editor itself is stage 7 (7.1 lays it out);
//! until then the window shows what the project holds and opens its folder.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::editor as text;
use dc_ui_kit::tokens::{FS_L, S2, S3, S5, SURFACE, TEXT, TEXT_2};
use dc_ui_kit::widgets::Button;
use eframe::egui::{self, RichText, Ui, vec2};

use crate::app::{Shared, lock, on_hud_monitor, other_window};

/// Size of the window at the start, points (v1: the whole work area, at least 800 × 600).
const SIZE: egui::Vec2 = vec2(1280.0, 800.0);
const MIN_SIZE: egui::Vec2 = vec2(800.0, 600.0);

/// The open project.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorWindow {
    pub root: PathBuf,
    pub name: String,
    pub clips: usize,
    /// Length of the project's clips, microseconds.
    pub length: i64,
    /// Frame size of the first clip, pixels.
    pub size: (u32, u32),
}

impl EditorWindow {
    pub fn new(recorded: &dc_capture::win::RecordedProject) -> Self {
        let clips = &recorded.project.clips;
        Self {
            root: recorded.dir.root().to_path_buf(),
            name: recorded.dir.name().to_owned(),
            clips: clips.len(),
            length: clips.iter().map(|c| c.media.duration.micros()).sum(),
            size: clips
                .first()
                .map_or((0, 0), |c| (c.media.size.width, c.media.size.height)),
        }
    }
}

/// Shows the editor while one is open; closing it brings the HUD back.
pub fn show(ctx: &egui::Context, shared: &Arc<Mutex<Shared>>) {
    let (title, corner) = {
        let shared = lock(shared);
        let Some(editor) = shared.editor.as_ref() else {
            return;
        };
        (
            text::title(&editor.name),
            on_hud_monitor(ctx, &shared, SIZE),
        )
    };
    let mut builder = egui::ViewportBuilder::default()
        .with_title(title)
        .with_inner_size(SIZE)
        .with_min_inner_size(MIN_SIZE);
    if let Some(corner) = corner {
        builder = builder.with_position(corner);
    }
    let shared = Arc::clone(shared);
    ctx.show_viewport_deferred(
        egui::ViewportId::from_hash_of("editor"),
        builder,
        move |ui, _| other_window(ui, &shared, frame),
    );
}

/// One frame of the editor window; closing it shows the HUD again.
fn frame(ui: &mut egui::Ui, shared: &mut Shared) {
    let Some(editor) = shared.editor.clone() else {
        return;
    };
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(SURFACE).inner_margin(S5))
        .show(ui, |ui| {
            if body(ui, &editor) {
                shared.report(crate::open_folder(&editor.root));
            }
        });
    if ui.input(|i| i.viewport().close_requested()) {
        shared.editor = None;
        shared.show_hud();
    }
}

/// The project's name and facts; true when «Открыть папку проекта» is pressed.
pub fn body(ui: &mut Ui, editor: &EditorWindow) -> bool {
    ui.label(
        RichText::new(&editor.name)
            .font(fonts::text(FS_L, Weight::Semibold))
            .color(TEXT),
    );
    ui.add_space(S2);
    let (w, h) = editor.size;
    let facts = text::facts(editor.clips, &crate::session::clock(editor.length), w, h);
    ui.label(RichText::new(facts).color(TEXT_2));
    ui.label(
        RichText::new(editor.root.display().to_string())
            .font(fonts::mono(12.0))
            .color(TEXT_2),
    );
    ui.add_space(S3);
    ui.add(Button::new(text::OPEN_FOLDER).small()).clicked()
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable;

    use super::*;

    #[test]
    fn opens_in_the_middle_of_the_hud_monitor() {
        let left = dc_capture::Area::new(-1920, 11, 1920, 1080).unwrap();
        assert_eq!(
            crate::app::centred(left, 1.0, SIZE),
            egui::pos2(-1600.0, 151.0)
        );
    }

    #[test]
    fn shows_the_recorded_project() {
        let editor = EditorWindow {
            root: PathBuf::from(r"C:\Users\User\Videos\DevCast\recording-3.dcproj"),
            name: "recording-3".to_owned(),
            clips: 1,
            length: 767_400_000,
            size: (1920, 1080),
        };
        let mut harness = dc_ui_kit::testing::harness()
            .with_size(vec2(520.0, 150.0))
            .build_ui_state(
                move |ui, opened: &mut bool| {
                    egui::Frame::new()
                        .fill(SURFACE)
                        .inner_margin(S5)
                        .show(ui, |ui| {
                            if body(ui, &editor) {
                                *opened = true;
                            }
                        });
                },
                false,
            );
        harness.run();
        harness.snapshot("editor_stub");
        harness.get_by_label("Клипов: 1 · 12:47 · 1920×1080");
        harness.get_by_label(text::OPEN_FOLDER).click();
        harness.run();
        assert!(*harness.state());
    }
}
