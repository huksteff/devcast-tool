//! The editor as a window of the app: opening it after a recording (the HUD hides), its frames
//! as a deferred viewport, the toasts of a frame, the timeline sizes kept in the settings, and
//! closing it (the HUD comes back).

use std::sync::{Arc, Mutex};
use std::time::Instant;

use dc_ui_kit::strings::editor as text;
use eframe::egui::{self, Ui};

use super::{EditorWindow, MIN_SIZE, SIZE, editor_ui};
use crate::app::{Shared, lock, on_hud_monitor, other_window};
use crate::settings::EditorLayout;

impl Shared {
    /// Opens the editor with the project a recording has just saved and hides the HUD; a
    /// project that cannot be opened (another writer holds it) is a toast.
    pub fn open_editor(&mut self, recorded: &dc_capture::win::RecordedProject) {
        match EditorWindow::new(recorded) {
            Ok((mut editor, problems)) => {
                if let Some(gpu) = self.gpu.clone() {
                    let started = editor.start_preview(gpu, self.ctx.clone());
                    self.report(started);
                }
                let sound = editor.start_sound();
                self.report(sound);
                let waves = editor.start_waves(self.ctx.clone());
                self.report(waves);
                for problem in problems {
                    self.report(Err(problem));
                }
                self.editor = Some(editor);
                self.hide_hud();
            }
            Err(error) => self.report(Err(error)),
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
        .with_icon(crate::logo::window_icon())
        .with_title(title)
        .with_inner_size(SIZE)
        .with_min_inner_size(MIN_SIZE);
    if let Some(corner) = corner {
        builder = builder.with_position(corner);
    }
    let id = egui::ViewportId::from_hash_of("editor");
    let window = Arc::clone(shared);
    ctx.show_viewport_deferred(id, builder, move |ui, _| other_window(ui, &window, frame));
    let news = lock(shared)
        .editor
        .as_ref()
        // Both are asked: each clears its own news.
        .is_some_and(|e| e.preview.take_news() | e.waves.take_news());
    if news {
        ctx.request_repaint_of(id);
    }
}

/// One frame of the editor window; closing it writes what is unsaved to the session and shows
/// the HUD again.
fn frame(ui: &mut Ui, shared: &mut Shared) {
    let layout = shared.store.get().editor;
    let keys = shared.store.get().editor_keys.clone();
    let Some(editor) = shared.editor.as_mut() else {
        return;
    };
    let now = Instant::now();
    let outcome = editor_ui(ui, editor, layout, &keys, now);
    let saved = editor.doc.autosave(now);
    if let Ok(Some(wait)) = saved {
        ui.ctx().request_repaint_after(wait);
    }
    let close = ui.input(|i| i.viewport().close_requested());
    let flushed = if close { editor.doc.flush() } else { Ok(()) };
    for error in outcome.errors {
        shared.report(Err(error));
    }
    shared.report(saved.map(drop));
    shared.report(flushed);
    if let Some(page) = outcome.open_settings {
        shared.open_settings_at(page);
    }
    save_layout(shared, ui, layout, outcome.layout);
    if close {
        shared.editor = None;
        shared.show_hud();
    }
}

/// Keeps a dragged timeline height (once the pointer is let go) and a new row factor.
fn save_layout(shared: &mut Shared, ui: &Ui, was: EditorLayout, now: EditorLayout) {
    let released = ui.input(|i| !i.pointer.any_down());
    let height = released && (now.timeline_height - was.timeline_height).abs() >= 1.0;
    let rows = (now.rows - was.rows).abs() > 1e-3;
    if height || rows {
        let saved = shared.store.update(|s| {
            if height {
                s.editor.timeline_height = now.timeline_height.round();
            }
            s.editor.rows = now.rows;
        });
        shared.report(saved);
    }
}
