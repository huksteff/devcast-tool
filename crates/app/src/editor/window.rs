//! The editor as a window of the app: opening it after a recording (the HUD hides), its frames
//! as a deferred viewport (and the timeline window, `timeline_window.rs`), the toasts of a
//! frame, the timeline sizes kept in the settings, and closing it (the HUD comes back).

use std::sync::{Arc, Mutex};
use std::time::Instant;

use dc_ui_kit::strings::editor as text;
use eframe::egui::{self, Ui};

use super::timeline_window::{self, Side};
use super::{EditorWindow, MIN_SIZE, SIZE, editor_ui};
use crate::app::{Shared, lock, on_hud_monitor, other_window};
use crate::settings::EditorLayout;

impl Shared {
    /// Opens the editor with the project a recording has just saved and hides the HUD; a
    /// project that cannot be opened (another writer holds it) is a toast.
    pub fn open_editor(&mut self, recorded: &dc_capture::win::RecordedProject) {
        match EditorWindow::new(recorded) {
            Ok((mut editor, problems)) => {
                let defaults = dc_project::settings::defaults_file()
                    .and_then(|file| dc_project::Defaults::load(&file));
                match defaults {
                    Ok(defaults) => editor.defaults = defaults,
                    Err(error) => self.report(Err(error)),
                }
                editor.captions = super::generation::Generation::new(
                    crate::models::Models::of_app(self.downloads.clone()),
                    crate::recognition::job::cli_worker(),
                );
                if let Some(gpu) = self.gpu.clone() {
                    let started = editor.start_preview(gpu, self.ctx.clone());
                    self.report(started);
                }
                let sound = editor.start_sound();
                self.report(sound);
                let waves = editor.start_waves(self.ctx.clone());
                self.report(waves);
                let presses = editor.start_presses(self.ctx.clone());
                self.report(presses);
                let streams = editor.start_streams(self.ctx.clone());
                self.report(streams);
                for problem in problems {
                    self.report(Err(problem));
                }
                super::timeline_window::restore(&mut editor, &self.store.get().editor);
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
    let (news, wake_editor, wake_timeline) = {
        let mut shared = lock(shared);
        let Some(e) = shared.editor.as_mut() else {
            return;
        };
        // All are asked: each clears its own news.
        let news = e.preview.take_news()
            | e.waves.take_news()
            | e.presses.take_news()
            | e.streams.take_news()
            | e.captions.take_news();
        let wake = &mut e.detached;
        (
            news,
            std::mem::take(&mut wake.wake_editor),
            std::mem::take(&mut wake.wake_timeline),
        )
    };
    if news || wake_editor {
        ctx.request_repaint_of(id);
    }
    super::timeline_window::show(ctx, shared, news || wake_timeline);
}

/// One frame of the editor window; closing it writes what is unsaved to the session and shows
/// the HUD again. If that fails, the window stays open with the error; closing again asks
/// whether to close without the edits (`closing.rs`).
pub(super) fn frame(ui: &mut Ui, shared: &mut Shared) {
    crate::borderless::opaque(ui);
    let layout = shared.store.get().editor;
    let keys = shared.store.get().editor_keys.clone();
    let groups = shared.store.get().inspector.clone();
    let model = shared.store.get().speech_model;
    let Some(editor) = shared.editor.as_mut() else {
        return;
    };
    editor.captions.model = model;
    let now = Instant::now();
    let before = editor.mark();
    let was_open = editor.detached.open;
    let outcome = editor_ui(ui, editor, layout, (&keys, &groups), now);
    editor.touched(ui.ctx(), Side::Editor, &before);
    let toggled = editor.detached.open != was_open;
    let saved = editor.doc.autosave(now);
    if let Ok(Some(wait)) = saved {
        ui.ctx().request_repaint_after(wait);
    }
    let close = ui.input(|i| i.viewport().close_requested());
    let flushed = close.then(|| editor.doc.flush());
    let discard = editor.closing.ask(ui);
    for error in outcome.errors {
        shared.report(Err(error));
    }
    shared.report(saved.map(drop));
    if let Some(page) = outcome.open_settings {
        shared.open_settings_at(page);
    }
    save_layout(shared, ui, layout, outcome.layout);
    if toggled {
        timeline_window::remember(shared);
    }
    if let Some(groups) = outcome.groups {
        let saved = shared.store.update(|s| s.inspector = groups);
        shared.report(saved);
    }
    match flushed {
        Some(Ok(())) => close_editor(shared),
        Some(Err(error)) => {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
            let first = shared.editor.as_mut().is_some_and(|e| e.closing.failed());
            if first {
                shared.report(Err(error));
            }
        }
        None if discard => close_editor(shared),
        None => {}
    }
}

fn close_editor(shared: &mut Shared) {
    timeline_window::remember(shared);
    shared.editor = None;
    shared.show_hud();
}

/// Keeps a dragged timeline height (once the pointer is let go) and a new row factor.
pub(super) fn save_layout(shared: &mut Shared, ui: &Ui, was: EditorLayout, now: EditorLayout) {
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

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
