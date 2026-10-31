//! The pointer on the timeline: the wheel (Ctrl — scale around the pointer, Shift — row heights,
//! otherwise time and, when the rows overflow, the rows), the ruler and the playhead (press
//! moves the playhead, drag scrubs, the view scrolls near the edges) and the navigation bar under
//! the rows (a thumb over the visible span).

use std::time::Duration;

use dc_core::OutputTime;
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{SURFACE, SURFACE_3, TEXT_2, TEXT_3};
use eframe::egui::{self, Rect, Response, Sense, Ui, Vec2, pos2};

use super::viewport::{Lanes, Viewport};
use crate::editor::{Action, EditorWindow, Transport};

/// Repaints while a drag presses an edge: the auto-scroll moves on without input.
const EDGE_TICK: Duration = Duration::from_millis(16);
/// Navigation bar: the gap around its track (mockup: 3 above and below, 8 on the right), the
/// least thumb width, points.
const NAV_GAP: f32 = 3.0;
const NAV_RIGHT: f32 = 8.0;
const THUMB_MIN: f32 = 24.0;

/// Takes the wheel over `area` (ruler, rows and bar) before the rows' scroll area sees it;
/// `overflow` — the rows are taller than their room, then the vertical wheel scrolls them.
pub fn wheel(ui: &Ui, area: Rect, lanes: &Lanes, view: &mut Viewport, rows: f32, overflow: bool) {
    if !ui.rect_contains_pointer(area) {
        return;
    }
    let (zoom, delta, pointer, shift) = ui.input(|i| {
        let shift = i.events.iter().rev().find_map(|event| match event {
            egui::Event::MouseWheel { modifiers, .. } => Some(modifiers.shift),
            _ => None,
        });
        (
            i.zoom_delta(),
            i.smooth_scroll_delta,
            i.pointer.hover_pos(),
            shift,
        )
    });
    // egui spreads a notch over a few frames: the rest belongs to the Shift of its notch, even
    // after Shift is let go.
    if let Some(shift) = shift {
        view.shift_wheel = shift;
    }
    if zoom != 1.0 {
        let x = pointer.map_or(lanes.left, |p| p.x);
        view.zoom_by(lanes, f64::from(zoom), x);
    }
    if delta == Vec2::ZERO {
        return;
    }
    let taken = if view.shift_wheel {
        // egui turns a Shift wheel horizontal; either way it is one wheel.
        view.wheel_rows(rows, delta.x + delta.y);
        Vec2::ZERO
    } else if overflow {
        view.scroll(lanes, delta.x);
        Vec2::new(0.0, delta.y)
    } else {
        view.scroll(lanes, delta.x + delta.y);
        Vec2::ZERO
    };
    ui.ctx().input_mut(|i| i.smooth_scroll_delta = taken);
}

/// The ruler and the playhead line: a press puts the playhead under the pointer, a drag scrubs
/// (playback pauses meanwhile and goes on after the release), a drag near an edge scrolls.
pub fn scrub(ui: &Ui, handles: &[&Response], lanes: &Lanes, editor: &mut EditorWindow) {
    let held = handles.iter().any(|r| r.is_pointer_button_down_on());
    let let_go = handles.iter().any(|r| r.clicked() || r.drag_stopped());
    let dragged = handles.iter().any(|r| r.dragged());
    let pointer = ui.input(|i| i.pointer.interact_pos());
    let view = &mut editor.view.timeline;
    if let (true, Some(at)) = (held || let_go, pointer) {
        if view.scrub.is_none() {
            view.scrub = Some(editor.playback.is_playing());
        }
        if dragged {
            let dt = ui.input(|i| i.stable_dt);
            if view.edge_scroll(lanes, at.x, dt) {
                ui.ctx().request_repaint_after(EDGE_TICK);
            }
        }
        let x = at.x.clamp(lanes.left, lanes.right());
        let end = OutputTime::ZERO + editor.tracks.duration;
        let t = view.scale(lanes).time(x).min(end);
        // A still pointer asks nothing: an action repaints, and idle frames must stay idle.
        if t != editor.view.playhead || editor.playback.is_playing() {
            editor.ask(Action::Transport(Transport::Scrub(t)));
        }
    }
    if !held && let Some(resume) = editor.view.timeline.scrub.take() {
        editor.ask(Action::Transport(Transport::Release(resume)));
    }
}

/// The navigation bar in `rect`: the thumb is the visible span; a press beside it centres the
/// view there, dragging moves the view.
pub fn nav(ui: &mut Ui, rect: Rect, lanes: &Lanes, view: &mut Viewport) {
    ui.painter().rect_filled(rect, 0, SURFACE);
    let track = Rect::from_min_max(
        pos2(lanes.left, rect.top() + NAV_GAP),
        pos2(rect.right() - NAV_RIGHT, rect.bottom() - NAV_GAP),
    );
    if track.width() <= 0.0 {
        return;
    }
    let response = ui
        .interact(
            track,
            ui.scope_id().with("timeline-nav"),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::Grab);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::ScrollBar, ui.is_enabled(), text::NAVIGATION)
    });
    let pressed = response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed());
    let pointer = ui.input(|i| i.pointer.interact_pos());
    if let (true, Some(at)) = (pressed, pointer) {
        if !thumb(track, lanes, view).contains(at) {
            view.centre_at(lanes, f64::from((at.x - track.left()) / track.width()));
        }
        view.grab = Some((at.x, view.start(lanes)));
    }
    match (response.is_pointer_button_down_on(), view.grab, pointer) {
        (true, Some((x, start)), Some(at)) => {
            let share = f64::from((at.x - x) / track.width());
            view.shift_from(lanes, start, share);
        }
        (false, Some(_), _) => view.grab = None,
        _ => {}
    }
    let fill = if response.hovered() || view.grab.is_some() {
        TEXT_2
    } else {
        TEXT_3
    };
    let painter = ui.painter();
    painter.rect_filled(track, 4, SURFACE_3);
    painter.rect_filled(thumb(track, lanes, view), 4, fill);
}

/// The thumb over `track`: at the start of the visible time, as wide as its share.
fn thumb(track: Rect, lanes: &Lanes, view: &Viewport) -> Rect {
    let (from, share) = view.thumb(lanes);
    let width = (share as f32 * track.width()).clamp(THUMB_MIN.min(track.width()), track.width());
    let left = (track.left() + from as f32 * track.width()).min(track.right() - width);
    Rect::from_x_y_ranges(left..=left + width, track.y_range())
}
