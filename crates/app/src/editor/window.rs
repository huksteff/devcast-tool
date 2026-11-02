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
    /// Opens the editor with the project a recording has just saved and hides the HUD.
    pub fn open_editor(&mut self, recorded: &dc_capture::win::RecordedProject) {
        self.open_project(recorded.dir.root(), true);
    }

    /// Gives a new editor the defaults, the speech model, the export queue and its threads.
    pub(super) fn start_editor(&mut self, editor: &mut EditorWindow) {
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
            if self.exports.is_none() {
                let ctx = self.ctx.clone();
                let wake = move || ctx.request_repaint_of(egui::ViewportId::ROOT);
                match crate::exports::Exports::of_gpu(gpu.gpu.clone(), wake) {
                    Ok(exports) => self.exports = Some(exports),
                    Err(error) => self.report(Err(error)),
                }
            }
            let started = editor.start_preview(gpu, self.ctx.clone());
            self.report(started);
        }
        if let Some(exports) = &self.exports {
            exports.untold();
            editor.exports = Some(exports.clone());
        }
        let sound = editor.start_sound();
        self.report(sound);
        let waves = editor.start_waves(self.ctx.clone());
        self.report(waves);
        let presses = editor.start_presses(self.ctx.clone());
        self.report(presses);
        let streams = editor.start_streams(self.ctx.clone());
        self.report(streams);
        let thumbs = editor.library.start(self.ctx.clone());
        self.report(thumbs);
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
            | e.captions.take_news()
            | e.library.thumbs.take_news()
            | e.exports.as_ref().is_some_and(|x| x.take_news());
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

/// One frame of the editor window. Closing it with unsaved changes asks first (`closing.rs`);
/// then the editor goes and the HUD comes back. Another project replaces it in the window
/// (`switch.rs`).
pub(super) fn frame(ui: &mut Ui, shared: &mut Shared) {
    crate::borderless::opaque(ui);
    let layout = shared.store.get().editor;
    let keys = shared.store.get().editor_keys.clone();
    let groups = shared.store.get().inspector.clone();
    let model = shared.store.get().speech_model;
    let speech = (
        shared.store.get().speech_language,
        shared.store.get().speech_tracks,
    );
    let recordings = shared.store.get().recordings_dir.clone();
    let recent = shared.store.get().recent_projects.clone();
    let Some(editor) = shared.editor.as_mut() else {
        return;
    };
    editor.captions.model = model;
    (editor.captions.language, editor.captions.listen) = speech;
    let open = editor.doc.dir().root().to_path_buf();
    editor.library.folder(&recordings, &open);
    editor.projects.recent = recent;
    if ui.input(|i| i.viewport().close_requested()) {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        editor.leave(super::Leave::Close);
    }
    let now = Instant::now();
    let before = editor.mark();
    let was_open = editor.detached.open;
    let outcome = editor_ui(ui, editor, layout, (&keys, &groups), now);
    editor.touched(ui.ctx(), Side::Editor, &before);
    let toggled = editor.detached.open != was_open;
    let chosen = (editor.captions.language, editor.captions.listen);
    let saved = editor.doc.autosave(now);
    if let Ok(Some(wait)) = saved {
        ui.ctx().request_repaint_after(wait);
    }
    for error in outcome.errors {
        shared.report(Err(error));
    }
    for done in outcome.done {
        let ctx = shared.ctx.clone();
        shared
            .toasts
            .push(&ctx, dc_ui_kit::widgets::ToastKind::Done, done);
    }
    shared.report(saved.map(drop));
    if let Some(page) = outcome.open_settings {
        shared.open_settings_at(page);
    }
    save_layout(shared, ui, layout, outcome.layout);
    if toggled {
        timeline_window::remember(shared);
    }
    if chosen != speech {
        let (language, tracks) = chosen;
        let saved = shared.store.update(|s| {
            s.speech_language = language;
            s.speech_tracks = tracks;
        });
        shared.report(saved);
    }
    if let Some(groups) = outcome.groups {
        let saved = shared.store.update(|s| s.inspector = groups);
        shared.report(saved);
    }
    if let Some(leave) = outcome.leave {
        shared.leave_project(leave);
    }
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
