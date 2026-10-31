//! The clips row, the magnetic main track (mockup 2c, 2e). «Выбор»: a click picks a clip, a
//! drag carries it to another place — a ghost follows the pointer, a white mark shows where it
//! goes in by the ghost's middle (v1), the release is `clip move`, Esc drops it back; the track
//! has no gaps, the other clips close up. «Лезвие»: a dashed line with scissors follows the
//! pointer, sticks to the playhead and the edges of the other rows' regions (with «Привязка»),
//! a click is `clip split` there. A click on a speed band picks its range and opens its speed;
//! a right click inside the picked range opens the range menu.

use std::time::Duration;

use dc_core::time::Output;
use dc_core::{ClipId, OutputTime, Range};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{BG, ICON, WHITE};
use eframe::egui::{self, CursorIcon, Rect, Response, Sense, Shape, Stroke, Ui, WidgetInfo};

use super::paint;
use super::scale::TimeScale;
use super::tracks::{Region, Row};
use super::viewport::Lanes;
use crate::editor::EditorWindow;
use crate::editor::montage;
use crate::editor::selection::Selected;
use crate::editor::view::{ClipDrag, RangeMenu, Tool};

/// The blade sticks to a target this close, points (mockup 2b: 8).
const SNAP: f32 = 8.0;
/// The ghost is lifted this much and keeps this opacity; the place the clip left keeps this
/// share of its colour (mockup `.drag`: .85; the left place .35).
const LIFT: f32 = 2.0;
const GHOST: f32 = 0.85;
const LEFT_PLACE: f32 = 0.35;
/// Width of the insertion mark (mockup `.ins`: 3, 2 from the row's edges).
const MARK: f32 = 3.0;
/// Repaints while a dragged clip presses an edge: the auto-scroll goes on without input.
const EDGE_TICK: Duration = Duration::from_millis(16);

/// A clip under the pointer: its place on the track, id, whole body and response.
struct Hit {
    place: usize,
    id: ClipId,
    body: Rect,
    response: Response,
}

/// The gestures and the overlays of the clips `lane`.
pub fn ui(ui: &mut Ui, lane: Rect, editor: &mut EditorWindow, scale: &TimeScale, lanes: &Lanes) {
    let clips: Vec<Region> = editor
        .tracks
        .rows
        .iter()
        .find(|t| t.row == Row::Clips)
        .map(|t| t.regions.clone())
        .unwrap_or_default();
    let background = ui.interact(lane, ui.scope_id().with("clips-lane"), Sense::click());
    let mut hits = Vec::new();
    for (place, region) in clips.iter().enumerate() {
        let Some(id) = region.clip else { continue };
        let body = paint::body(lane, scale, region.range);
        let shown = body.intersect(lane);
        if !shown.is_positive() {
            continue;
        }
        let key = ui.scope_id().with(("clip", id.number()));
        let response = ui.interact(shown, key, Sense::click_and_drag());
        response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, &region.label));
        hits.push(Hit {
            place,
            id,
            body,
            response,
        });
    }
    let mut menu = background.secondary_clicked();
    match editor.view.tool {
        Tool::Select => {
            speed_bands(ui, lane, editor, scale);
            select(ui, editor, &hits, &background);
        }
        Tool::Blade => blade(ui, lane, editor, scale, &hits),
    }
    menu |= hits.iter().any(|h| h.response.secondary_clicked());
    let pointer = ui.input(|i| i.pointer.interact_pos());
    let inside = |range: Range<Output>| pointer.is_some_and(|p| range.contains(scale.time(p.x)));
    if menu && editor.view.range.is_some_and(inside) {
        editor.view.menu = Some(RangeMenu::Actions);
    }
    let painter = ui.painter_at(lane);
    for hit in &hits {
        let picked = editor.view.selection.contains(Selected::Clip(hit.id));
        let dragged = editor.view.drag.is_some_and(|d| d.clip == hit.id);
        if picked && !dragged {
            paint::selected(&painter, hit.body);
        }
    }
    drag(ui, lane, editor, scale, lanes, &clips);
}

/// A click picks the clip, a click on no clip drops the selection, a drag picks it up.
fn select(ui: &Ui, editor: &mut EditorWindow, hits: &[Hit], background: &Response) {
    for hit in hits {
        let response = &hit.response;
        let response = response.clone().on_hover_cursor(CursorIcon::Grab);
        if response.clicked() || response.drag_started() {
            editor.view.selection.only(Selected::Clip(hit.id));
        }
        if response.drag_started() {
            let x = ui
                .input(|i| i.pointer.press_origin())
                .map_or(hit.body.left(), |p| p.x);
            editor.view.drag = Some(ClipDrag {
                clip: hit.id,
                grab: x - hit.body.left(),
                from: hit.place,
            });
        }
    }
    if background.clicked() {
        editor.view.selection.clear();
    }
}

/// Speed bands over the clips: a click picks the band's range and opens its speed.
fn speed_bands(ui: &Ui, lane: Rect, editor: &mut EditorWindow, scale: &TimeScale) {
    for band in editor.tracks.speeds.clone() {
        let rect = paint::speed_band(lane, scale, band.region.range).intersect(lane);
        if !rect.is_positive() {
            continue;
        }
        let key = ui
            .scope_id()
            .with(("speed", band.region.range.start().micros()));
        let response = ui
            .interact(rect, key, Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let name = text::speed_band(band.factor);
        response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, &name));
        if response.clicked() {
            editor.view.range = Some(band.region.range);
            editor.view.menu = Some(RangeMenu::Speed { own: band.factor });
        }
    }
}

/// The blade line under the pointer; a click on a clip cuts it there.
fn blade(ui: &Ui, lane: Rect, editor: &mut EditorWindow, scale: &TimeScale, hits: &[Hit]) {
    let shown = lane.intersect(ui.clip_rect());
    let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    if !shown.contains(pointer) {
        return;
    }
    let at = blade_time(editor, scale, pointer.x);
    let x = scale.x(at);
    let painter = ui.painter_at(lane);
    let stroke = Stroke::new(1.5, WHITE);
    let (top, bottom) = (egui::pos2(x, lane.top()), egui::pos2(x, lane.bottom()));
    painter.extend(Shape::dashed_line(&[top, bottom], stroke, 3.0, 3.0));
    let icon = Rect::from_min_size(
        egui::pos2(x + 4.0, lane.center().y - ICON / 2.0),
        egui::vec2(ICON, ICON),
    );
    Icon::Blade.paint(&painter, icon, WHITE);
    ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
    if hits.iter().any(|h| h.response.clicked()) {
        editor.ask(montage::split(at));
    }
}

/// The moment the blade at x cuts: the playhead or an edge of another row's region within
/// `SNAP` while «Привязка» is on (not on a clip edge), else the time under x.
fn blade_time(editor: &EditorWindow, scale: &TimeScale, x: f32) -> OutputTime {
    let free = scale.time(x).min(editor.end());
    if !editor.view.magnet {
        return free;
    }
    let edges = editor
        .tracks
        .rows
        .iter()
        .filter(|t| t.row != Row::Clips)
        .flat_map(|t| &t.regions)
        .flat_map(|r| [r.range.start(), r.range.end()]);
    // A clip edge is no place to cut: `clip split` refuses it.
    let joins: Vec<OutputTime> = editor
        .tracks
        .rows
        .iter()
        .filter(|t| t.row == Row::Clips)
        .flat_map(|t| &t.regions)
        .flat_map(|r| [r.range.start(), r.range.end()])
        .collect();
    let targets = std::iter::once(editor.view.playhead).chain(edges);
    snap(x, scale, targets.filter(|t| !joins.contains(t))).unwrap_or(free)
}

/// The target nearest to x on `scale` within `SNAP`.
pub fn snap(
    x: f32,
    scale: &TimeScale,
    targets: impl Iterator<Item = OutputTime>,
) -> Option<OutputTime> {
    targets
        .map(|t| (t, (scale.x(t) - x).abs()))
        .filter(|(_, distance)| *distance <= SNAP)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(t, _)| t)
}

/// The place, from 0, a dragged clip takes among the `others` (output ranges in track order):
/// before the first whose middle is after the ghost's middle `centre`.
pub fn slot(others: &[Range<Output>], centre: OutputTime) -> usize {
    others
        .iter()
        .take_while(|r| (r.start().micros() + r.end().micros()) / 2 < centre.micros())
        .count()
}

/// A dragged clip: the ghost, the mark, the auto-scroll at the edges; the release moves it.
fn drag(
    ui: &Ui,
    lane: Rect,
    editor: &mut EditorWindow,
    scale: &TimeScale,
    lanes: &Lanes,
    clips: &[Region],
) {
    let Some(drag) = editor.view.drag else {
        return;
    };
    let Some(dragged) = clips.iter().find(|r| r.clip == Some(drag.clip)) else {
        editor.view.drag = None;
        return;
    };
    let others: Vec<Range<Output>> = clips
        .iter()
        .filter(|r| r.clip != Some(drag.clip))
        .map(|r| r.range)
        .collect();
    let (down, pointer, dt) = ui.input(|i| {
        let p = &i.pointer;
        (p.primary_down(), p.interact_pos(), i.stable_dt)
    });
    let Some(at) = pointer else { return };
    let body = paint::body(lane, scale, dragged.range);
    let left = at.x - drag.grab;
    let place = slot(&others, scale.time(left + body.width() / 2.0));
    if !down {
        editor.view.drag = None;
        if place != drag.from {
            editor.ask(montage::move_to(drag.clip, place + 1));
        }
        return;
    }
    if editor.view.timeline.edge_scroll(lanes, at.x, dt) {
        ui.ctx().request_repaint_after(EDGE_TICK);
    }
    let painter = ui.painter_at(lane);
    painter.rect_filled(body, 4, BG.gamma_multiply(1.0 - LEFT_PLACE));
    let mark = match place {
        0 => others.first().map(|r| scale.x(r.start())),
        _ => others.get(place - 1).map(|r| scale.x(r.end())),
    };
    let ghost = body.translate(egui::vec2(left - body.left(), -LIFT));
    let mut lifted = painter.clone();
    lifted.multiply_opacity(GHOST);
    paint::region_body(&lifted, ghost, dragged, paint::hue(Row::Clips));
    paint::selected(&painter, ghost);
    // Over the ghost and inside the lane, so the place stays visible.
    if let Some(x) = mark {
        let half = MARK / 2.0;
        let x = x.clamp(lane.left() + half, lane.right() - half);
        let ys = (lane.top() + 2.0)..=(lane.bottom() - 2.0);
        painter.rect_filled(Rect::from_x_y_ranges((x - half)..=(x + half), ys), 2, WHITE);
    }
    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
}
