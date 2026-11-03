//! The timeline in its own window (7.26, docs/design/editor.html, mockup 7): a deferred viewport
//! with its own toolbar over the same editor. The project, the history, the playhead, the
//! selection and the tools are the editor's (`Document`, `View`): an edit here is the same
//! registry command and the same undo step. Only the scale and scroll are its own (v1): the
//! viewport of the timeline shown is `View::timeline`, the hidden one waits in `View::parked`.
//! While the window is open the editor hides its docked timeline.
//!
//! Threads wake only the main window (decision 002), and so do these windows: a frame that
//! changed what the other window shows marks it (`Detached::wake_*`) and repaints the main
//! window, whose pass repaints the marked window (`editor::window::show`).

use std::sync::{Arc, Mutex};
use std::time::Instant;

use dc_capture::MonitorInfo;
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::SURFACE;
use eframe::egui::{self, Frame, Ui, Vec2, pos2, vec2};

use super::timeline::{self, Place};
use super::{EditorWindow, View, actions, keys, playback};
use crate::app::{Shared, lock, other_window};
use crate::settings::{EditorLayout, WindowPlace};

pub const ID: &str = "editor-timeline";
/// Size of the window at its first opening and its least size, points (mockup 7, v1).
const SIZE: Vec2 = vec2(1280.0, 460.0);
const MIN_SIZE: Vec2 = vec2(720.0, 260.0);

#[derive(Debug, Default)]
pub struct Detached {
    /// The timeline window is open.
    pub open: bool,
    /// Where the window is now; kept in the settings when it closes or the editor does.
    pub place: Option<WindowPlace>,
    /// A frame of the timeline window changed what the editor shows: it repaints.
    pub wake_editor: bool,
    /// A frame of the editor changed what the timeline window shows: it repaints.
    pub wake_timeline: bool,
}

/// The window whose frame ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Editor,
    Timeline,
}

/// What both windows show: the view without the windows' own viewports, the playhead drag,
/// playback, the open window and the revision of the project.
#[derive(Debug, PartialEq)]
pub struct Mark {
    view: View,
    scrub: bool,
    playing: bool,
    open: bool,
    revision: u64,
}

impl EditorWindow {
    pub fn mark(&self) -> Mark {
        let mut view = self.view.clone();
        let scrub = view.timeline.scrub.is_some();
        view.timeline = timeline::Viewport::default();
        view.parked = timeline::Viewport::default();
        let playing = self.playback.is_playing();
        if playing {
            // Each window moves the playhead itself while playing (`playback::advance`); marking
            // the other one for it would repaint both without end.
            view.playhead = dc_core::OutputTime::ZERO;
        }
        Mark {
            view,
            scrub,
            playing,
            open: self.detached.open,
            revision: self.preview.project().1,
        }
    }

    /// After a frame of `side` that began at `before`: while the timeline window is (or was)
    /// open and something both show changed, the other window is marked and the main window
    /// woken to repaint it.
    pub fn touched(&mut self, ctx: &egui::Context, side: Side, before: &Mark) {
        let after = self.mark();
        if !(before.open || after.open) || after == *before {
            return;
        }
        match side {
            Side::Editor => self.detached.wake_timeline = true,
            Side::Timeline => self.detached.wake_editor = true,
        }
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }

    /// Opens the timeline window (`true`) or brings the timeline back under the preview; each
    /// place keeps its own viewport.
    pub fn detach(&mut self, open: bool) {
        if self.detached.open != open {
            self.detached.open = open;
            std::mem::swap(&mut self.view.timeline, &mut self.view.parked);
        }
    }

    /// Takes what the threads sent: peaks, sound tracks, presses (which rebuild the rows), the
    /// pictures of the library and the projects menu. Either window's frame takes it, whichever
    /// runs first.
    pub(super) fn receive(&mut self) {
        self.waves.receive();
        self.library.thumbs.receive();
        self.streams.receive();
        if self.presses.receive() {
            self.retrack();
        }
    }
}

/// The open state and the place of the timeline window go to the settings.
pub fn remember(shared: &mut Shared) {
    let Some(editor) = shared.editor.as_ref() else {
        return;
    };
    let (open, place) = (editor.detached.open, editor.detached.place);
    let saved = shared.store.update(|s| {
        s.editor.timeline_window = open;
        if place.is_some() {
            s.editor.timeline_place = place;
        }
    });
    shared.report(saved);
}

/// The editor opens with the timeline window as it was left (`layout` of the settings).
pub fn restore(editor: &mut EditorWindow, layout: &EditorLayout) {
    editor.detach(layout.timeline_window);
    editor.detached.place = layout.timeline_place;
}

/// Shows the timeline window while it is open, from the main window's pass; `repaint` — a
/// thread or the editor changed what it shows.
pub fn show(ctx: &egui::Context, shared: &Arc<Mutex<Shared>>, repaint: bool) {
    let (title, size, corner) = {
        let shared = lock(shared);
        let Some(editor) = shared.editor.as_ref().filter(|e| e.detached.open) else {
            return;
        };
        let ppp = ctx.native_pixels_per_point().unwrap_or(1.0);
        let saved = shared.store.get().editor.timeline_place;
        let (size, corner) = match on_screen(saved, &shared.sources.monitors, ppp) {
            Some(p) => (vec2(p.width, p.height), Some(pos2(p.x, p.y))),
            None => (SIZE, crate::monitor::for_timeline(ctx, &shared, SIZE)),
        };
        (text::window_title(&editor.name), size, corner)
    };
    let mut builder = egui::ViewportBuilder::default()
        .with_icon(crate::logo::window_icon())
        .with_title(title)
        .with_inner_size(size)
        .with_min_inner_size(MIN_SIZE);
    if let Some(corner) = corner {
        builder = builder.with_position(corner);
    }
    let id = egui::ViewportId::from_hash_of(ID);
    let window = Arc::clone(shared);
    ctx.show_viewport_deferred(id, builder, move |ui, _| other_window(ui, &window, frame));
    if repaint {
        ctx.request_repaint_of(id);
    }
}

/// The saved place while its middle is on one of `monitors` (`ppp` physical pixels a point).
pub fn on_screen(
    place: Option<WindowPlace>,
    monitors: &[MonitorInfo],
    ppp: f32,
) -> Option<WindowPlace> {
    place.filter(|p| {
        let x = (p.x + p.width / 2.0) * ppp;
        let y = (p.y + p.height / 2.0) * ppp;
        let middle = (x.round() as i32, y.round() as i32);
        monitors
            .iter()
            .any(|m| crate::hud::placement::contains(&m.bounds, middle))
    })
}

/// One frame of the timeline window: the shortcuts work here too (mockup 7, v2), its actions run
/// as the editor's; closing it brings the timeline back under the preview.
pub fn frame(ui: &mut Ui, shared: &mut Shared) {
    crate::borderless::opaque(ui);
    let layout = shared.store.get().editor;
    let Some(editor) = shared.editor.as_mut() else {
        return;
    };
    let before = editor.mark();
    let now = Instant::now();
    editor.receive();
    keys::run(ui, editor);
    let sound_error = playback::advance(editor, now);
    let rows = egui::CentralPanel::no_frame()
        .frame(Frame::new().fill(SURFACE))
        .show(ui, |ui| {
            timeline::ui(ui, editor, layout.rows, Place::Window)
        })
        .inner;
    let mut errors: Vec<String> = sound_error.into_iter().collect();
    errors.extend(actions::run(ui, editor, now));
    playback::keep_playing(ui, editor);
    editor.detached.place = ui.input(|i| {
        let viewport = i.viewport();
        let (outer, inner) = (viewport.outer_rect?, viewport.inner_rect?);
        Some(WindowPlace {
            x: outer.min.x,
            y: outer.min.y,
            width: inner.width(),
            height: inner.height(),
        })
    });
    let closed = ui.input(|i| i.viewport().close_requested());
    if closed {
        editor.detach(false);
    }
    editor.touched(ui.ctx(), Side::Timeline, &before);
    for error in errors {
        shared.report(Err(error));
    }
    super::window::save_layout(shared, ui, layout, EditorLayout { rows, ..layout });
    if closed {
        remember(shared);
    }
}
