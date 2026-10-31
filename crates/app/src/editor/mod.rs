//! The editor window (docs/design/editor.html, «Главное окно редактора»): header, preview with
//! its transport, inspector on the right, timeline under them. It opens with the project of a
//! finished recording; the HUD hides meanwhile and comes back when the editor closes (v1).
//! The project is read-only here until the editor is wired to the command registry (7.2).

mod header;
mod inspector;
mod preview;
mod timeline;
mod transport;
mod view;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use dc_core::{PixelSize, Project};
use dc_fx_crop::{Aspect, Crop};
use dc_ui_kit::strings::editor as text;
use dc_ui_kit::tokens::{BG, SURFACE};
use eframe::egui::{self, Frame, Ui, vec2};

use crate::app::{Shared, lock, on_hud_monitor, other_window};
pub use timeline::Tracks;
pub use view::View;

/// Size of the window at the start and its least size, points (mockup).
const SIZE: egui::Vec2 = vec2(1280.0, 800.0);
const MIN_SIZE: egui::Vec2 = vec2(1024.0, 640.0);
/// Height of the header, width of the inspector (mockup).
const HEADER: f32 = 44.0;
const INSPECTOR: f32 = 340.0;
/// The timeline is dragged between this height and 75 % of the window (v1).
const TIMELINE_MIN: f32 = 140.0;
const TIMELINE_MAX_SHARE: f32 = 0.75;

/// The open project.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorWindow {
    pub root: PathBuf,
    pub name: String,
    pub project: Project,
    /// The project has changes not saved with Ctrl+S (its `session.json` exists).
    pub unsaved: bool,
    /// Sound tracks the recordings have (`mic`, `system`): rows of the source sound.
    pub sound: Vec<String>,
    /// The timeline rows of `project`.
    pub tracks: Tracks,
    /// Output frame: its aspect setting and size (none without clips).
    pub aspect: Aspect,
    pub canvas: Option<PixelSize>,
    pub view: View,
}

impl EditorWindow {
    pub fn new(recorded: &dc_capture::win::RecordedProject) -> Self {
        let sound = recorded
            .summary
            .audio
            .iter()
            .map(|a| a.track.to_owned())
            .collect();
        Self::of(
            recorded.dir.root().to_path_buf(),
            // The folder is `name.dcproj`; the header shows the extension apart.
            std::path::Path::new(recorded.dir.name())
                .file_stem()
                .map_or_else(
                    || recorded.dir.name().to_owned(),
                    |s| s.to_string_lossy().into_owned(),
                ),
            recorded.project.clone(),
            recorded.dir.has_unsaved_changes(),
            sound,
        )
    }

    /// The editor of `project` in folder `root`.
    pub fn of(
        root: PathBuf,
        name: String,
        project: Project,
        unsaved: bool,
        sound: Vec<String>,
    ) -> Self {
        let mut tracks = Tracks::of(&project, &sound);
        let crop = project.state::<Crop>().unwrap_or_else(|e| {
            tracks.problems.push(e.to_string());
            Default::default()
        });
        Self {
            canvas: dc_fx_crop::canvas(&project, &crop.settings),
            aspect: crop.settings.aspect,
            root,
            name,
            project,
            unsaved,
            sound,
            tracks,
            view: View::default(),
        }
    }
}

/// What a frame of the editor asks of the app.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outcome {
    pub open_settings: bool,
    /// Height of the timeline after the frame, points.
    pub timeline_height: f32,
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
    let shared = Arc::clone(shared);
    ctx.show_viewport_deferred(
        egui::ViewportId::from_hash_of("editor"),
        builder,
        move |ui, _| other_window(ui, &shared, frame),
    );
}

/// One frame of the editor window; closing it shows the HUD again.
fn frame(ui: &mut Ui, shared: &mut Shared) {
    let height = shared.store.get().editor.timeline_height;
    let Some(editor) = shared.editor.as_mut() else {
        return;
    };
    let outcome = editor_ui(ui, editor, height);
    if outcome.open_settings {
        shared.open_settings();
    }
    let released = ui.input(|i| !i.pointer.any_down());
    if released && (outcome.timeline_height - height).abs() >= 1.0 {
        let saved = shared
            .store
            .update(|s| s.editor.timeline_height = outcome.timeline_height.round());
        shared.report(saved);
    }
    if ui.input(|i| i.viewport().close_requested()) {
        shared.editor = None;
        shared.show_hud();
    }
}

/// The whole window: header, timeline at the bottom (`timeline_height` until dragged),
/// inspector on the right, preview in the middle.
pub fn editor_ui(ui: &mut Ui, editor: &mut EditorWindow, timeline_height: f32) -> Outcome {
    let mut open_settings = false;
    egui::Panel::top("editor-header")
        .exact_size(HEADER)
        .frame(Frame::new().fill(SURFACE))
        .show(ui, |ui| open_settings = header::ui(ui, editor));
    let most = (ui.available_height() * TIMELINE_MAX_SHARE).max(TIMELINE_MIN);
    let timeline = egui::Panel::bottom("editor-timeline")
        .resizable(true)
        .default_size(timeline_height.clamp(TIMELINE_MIN, most))
        .size_range(TIMELINE_MIN..=most)
        .frame(Frame::new().fill(SURFACE))
        .show(ui, |ui| timeline::ui(ui, editor));
    egui::Panel::right("editor-inspector")
        .exact_size(INSPECTOR)
        .frame(Frame::new().fill(SURFACE))
        .show(ui, |ui| inspector::ui(ui, &mut editor.view));
    egui::CentralPanel::no_frame()
        .frame(Frame::new().fill(BG))
        .show(ui, |ui| preview::ui(ui, editor));
    Outcome {
        open_settings,
        timeline_height: timeline.response.rect.height(),
    }
}
