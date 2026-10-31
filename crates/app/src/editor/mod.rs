//! The editor window (docs/design/editor.html, «Главное окно редактора»): header, preview with
//! its transport, inspector on the right, timeline under them. It opens with the project of a
//! finished recording; the HUD hides meanwhile and comes back when the editor closes (v1).
//! Controls change the project only by queueing an `Action` (a registry command, undo, redo);
//! the frame runs them after its layout and rebuilds the view from the new project.

mod actions;
mod document;
mod header;
mod inspector;
mod preview;
mod timeline;
mod transport;
mod view;

#[cfg(test)]
mod editing_tests;
#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use dc_core::PixelSize;
use dc_fx_crop::{Aspect, Crop};
use dc_project::ProjectDir;
use dc_ui_kit::strings::editor as text;
use dc_ui_kit::tokens::{BG, SURFACE};
use eframe::egui::{self, Frame, Ui, vec2};

use crate::app::{Shared, lock, on_hud_monitor, other_window};
pub use actions::Action;
pub use document::{Document, DocumentError, LOCK_OWNER, Way};
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

/// The open project and what the window shows of it.
#[derive(Debug)]
pub struct EditorWindow {
    pub doc: Document,
    /// Project name: the folder without `.dcproj`.
    pub name: String,
    /// Sound tracks the recordings have (`mic`, `system`): rows of the source sound.
    pub sound: Vec<String>,
    /// The timeline rows of the project.
    pub tracks: Tracks,
    /// Output frame: its aspect setting and size (none without clips).
    pub aspect: Aspect,
    pub canvas: Option<PixelSize>,
    pub view: View,
    /// Actions the controls asked for in this frame.
    actions: Vec<Action>,
}

impl EditorWindow {
    /// The editor of the project a recording has just saved.
    pub fn new(
        recorded: &dc_capture::win::RecordedProject,
    ) -> Result<(Self, Vec<String>), DocumentError> {
        let sound = recorded
            .summary
            .audio
            .iter()
            .map(|a| a.track.to_owned())
            .collect();
        Self::open(recorded.dir.clone(), sound)
    }

    /// Opens the project in `dir` for editing; also returns the problems to show (a broken
    /// history, a broken effect state).
    pub fn open(dir: ProjectDir, sound: Vec<String>) -> Result<(Self, Vec<String>), DocumentError> {
        let (doc, mut problems) = Document::open(dir)?;
        // The folder is `name.dcproj`; the header shows the extension apart.
        let name = std::path::Path::new(doc.dir().name())
            .file_stem()
            .map_or_else(
                || doc.dir().name().to_owned(),
                |s| s.to_string_lossy().into_owned(),
            );
        let mut editor = Self {
            doc,
            name,
            sound,
            tracks: Tracks::default(),
            aspect: Aspect::default(),
            canvas: None,
            view: View::default(),
            actions: Vec::new(),
        };
        editor.refresh();
        problems.extend(editor.tracks.problems.iter().cloned());
        Ok((editor, problems))
    }

    /// Rebuilds the view of the project after it changed.
    fn refresh(&mut self) {
        let project = self.doc.project();
        let mut tracks = Tracks::of(project, &self.sound);
        let crop = project.state::<Crop>().unwrap_or_else(|e| {
            tracks.problems.push(e.to_string());
            Default::default()
        });
        self.canvas = dc_fx_crop::canvas(project, &crop.settings);
        self.aspect = crop.settings.aspect;
        self.tracks = tracks;
    }

    /// Queues `action` to run after this frame's layout.
    pub fn ask(&mut self, action: Action) {
        self.actions.push(action);
    }
}

impl Shared {
    /// Opens the editor with the project a recording has just saved and hides the HUD; a
    /// project that cannot be opened (another writer holds it) is a toast.
    pub fn open_editor(&mut self, recorded: &dc_capture::win::RecordedProject) {
        match EditorWindow::new(recorded) {
            Ok((editor, problems)) => {
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

/// What a frame of the editor asks of the app.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub open_settings: bool,
    /// Height of the timeline after the frame, points.
    pub timeline_height: f32,
    /// Why an action failed, for a toast.
    pub errors: Vec<String>,
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

/// One frame of the editor window; closing it writes what is unsaved to the session and shows
/// the HUD again.
fn frame(ui: &mut Ui, shared: &mut Shared) {
    let height = shared.store.get().editor.timeline_height;
    let Some(editor) = shared.editor.as_mut() else {
        return;
    };
    let now = Instant::now();
    let outcome = editor_ui(ui, editor, height, now);
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
    if close {
        shared.editor = None;
        shared.show_hud();
    }
}

/// The whole window: header, timeline at the bottom (`timeline_height` until dragged),
/// inspector on the right, preview in the middle; then the actions of the frame, at `now`.
pub fn editor_ui(
    ui: &mut Ui,
    editor: &mut EditorWindow,
    timeline_height: f32,
    now: Instant,
) -> Outcome {
    if let Some(action) = actions::shortcut(ui) {
        editor.ask(action);
    }
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
    let errors = actions::run(ui, editor, now);
    Outcome {
        open_settings,
        timeline_height: timeline.response.rect.height(),
        errors,
    }
}
