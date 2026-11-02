//! The regions of the effect rows in «Выбор» (mockup 2a, 2b): one way for zooms, captions,
//! callout events, annotations and extra sound. A click picks a region, Ctrl+click adds or takes
//! it out, a click on an empty lane drops the selection — or adds the ghost's region where one
//! shows (`ghost.rs`); a drag from an empty lane draws a rubber band. A drag of a region carries all picked regions, a drag of an edge (`EDGE` inside
//! each end) changes the length; the edges of the speed bands on the clips row drag the same
//! way. What follows the pointer and what the release asks for is `gesture.rs`. A click on the
//! stretch of a clip on «Микрофон» / «Системный звук» picks the clip (its sound, 7.19).

use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::inspector::on_row;
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{BORDER, FS_S, R1, SURFACE, TEXT};
use eframe::egui::{
    self, Align2, CursorIcon, Id, LayerId, Order, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    WidgetInfo, vec2,
};

use super::gesture;
use super::paint;
use super::scale::TimeScale;
use super::tracks::{Region, Row};
use super::viewport::Lanes;
use crate::editor::EditorWindow;
use crate::editor::selection::{Item, Selected};
use crate::editor::view::{Gesture, Target, Tool};
use crate::keymap::{EditorKey, Label};

/// The edge grab zone inside each end of a region, points, at most a third of its width so the
/// middle still moves it (v1 had 28 for long regions).
const EDGE: f32 = 8.0;

/// The gestures on the regions of `rows` (each row with its lane, in the order of
/// `Tracks::rows`).
pub fn ui(
    ui: &mut Ui,
    rows: &[(Row, Rect)],
    editor: &mut EditorWindow,
    scale: &TimeScale,
    lanes: &Lanes,
) {
    if editor.view.tool != Tool::Select {
        return;
    }
    let ctrl = ui.input(|i| i.modifiers.command);
    let press = ui.input(|i| i.pointer.press_origin());
    for (index, &(row, lane)) in rows.iter().enumerate() {
        if row == Row::Clips {
            continue;
        }
        let empty = ui.interact(
            lane,
            ui.scope_id().with(("lane", index)),
            Sense::click_and_drag(),
        );
        let added = super::ghost::ui(ui, editor, row, lane, &empty, scale);
        if empty.clicked() && !ctrl && !added {
            editor.view.selection.clear();
        }
        if let (true, Some(from)) = (empty.drag_started(), press) {
            let base = match ctrl {
                true => editor.view.selection.clone(),
                false => Default::default(),
            };
            editor.view.gesture = Some(Gesture::Band { from, base });
        }
    }
    for (index, &(row, lane)) in rows.iter().enumerate() {
        let regions = editor.tracks.rows[index].regions.clone();
        for region in regions {
            if let (None, Some(clip), false) = (region.item, region.clip, row == Row::Clips) {
                clip_sound(ui, editor, (row, lane, scale), &region, (clip, ctrl));
                continue;
            }
            let Some(item) = region.item else { continue };
            let body = paint::body(lane, scale, region.range);
            if !body.intersect(lane).is_positive() {
                continue;
            }
            let id = ui.scope_id().with(("region", item));
            let response = ui
                .interact(body.intersect(lane), id, Sense::click_and_drag())
                .on_hover_cursor(CursorIcon::Grab);
            response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, &region.label));
            pick(editor, &response, item, ctrl, press, scale, index);
            for end in [false, true] {
                let zone = edge_zone(ui, body, lane, id.with(end), end);
                if zone.drag_started() {
                    editor.view.selection.only(Selected::Region(item));
                    let target = Target::Region(item);
                    editor.view.gesture = Some(Gesture::Edge { target, end });
                }
            }
        }
    }
    speed_edges(ui, rows, editor, scale);
    gesture::run(ui, rows, editor, scale, lanes);
    rings(ui, rows, editor, scale);
}

/// A click or a drag on the region of `item` on row `index`.
fn pick(
    editor: &mut EditorWindow,
    response: &Response,
    item: Item,
    ctrl: bool,
    press: Option<egui::Pos2>,
    scale: &TimeScale,
    index: usize,
) {
    let selected = Selected::Region(item);
    if response.clicked() {
        match ctrl {
            true => editor.view.selection.toggle(selected),
            false => editor.view.selection.only(selected),
        }
    }
    if response.drag_started() {
        if !editor.view.selection.contains(selected) {
            match ctrl {
                true => editor.view.selection.add(selected),
                false => editor.view.selection.only(selected),
            }
        }
        let grab = scale.time(press.map_or(0.0, |p| p.x));
        editor.view.gesture = Some(Gesture::Move {
            grab,
            row: index,
            item,
        });
    }
}

/// The stretch of a clip on «Микрофон» / «Системный звук»: a click picks the clip (its panel has
/// the clip's sound, 7.19), Ctrl+click adds or takes it out. It does not move: the clips row
/// orders the clips.
fn clip_sound(
    ui: &Ui,
    editor: &mut EditorWindow,
    (row, lane, scale): (Row, Rect, &TimeScale),
    region: &Region,
    (clip, ctrl): (dc_core::ClipId, bool),
) {
    let body = paint::body(lane, scale, region.range).intersect(lane);
    if !body.is_positive() {
        return;
    }
    let id = ui.scope_id().with(("clip-sound", clip, lane.top() as i32));
    let response = ui.interact(body, id, Sense::click());
    let name = on_row(&super::name(row), &text::clip(clip.number()));
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, &name));
    if response.clicked() {
        let picked = Selected::Clip(clip);
        match ctrl {
            true => editor.view.selection.toggle(picked),
            false => editor.view.selection.only(picked),
        }
    }
}

/// The grab zone of the start or `end` edge of `body` on `lane`.
fn edge_zone(ui: &Ui, body: Rect, lane: Rect, id: Id, end: bool) -> Response {
    ui.interact(side(body, end).intersect(lane), id, Sense::drag())
        .on_hover_cursor(CursorIcon::ResizeHorizontal)
}

/// The edges of the speed bands of the clips row.
fn speed_edges(ui: &Ui, rows: &[(Row, Rect)], editor: &mut EditorWindow, scale: &TimeScale) {
    let Some(&(_, lane)) = rows.iter().find(|(row, _)| *row == Row::Clips) else {
        return;
    };
    for band in editor.tracks.speeds.clone() {
        let rect = paint::speed_band(lane, scale, band.region.range);
        let key = ui
            .scope_id()
            .with(("speed-edge", band.region.range.start().micros()));
        for end in [false, true] {
            let zone = side(rect, end);
            let response = ui
                .interact(zone.intersect(lane), key.with(end), Sense::drag())
                .on_hover_cursor(CursorIcon::ResizeHorizontal);
            if response.drag_started() {
                let target = Target::Speed {
                    range: band.region.range,
                    factor: band.factor,
                };
                editor.view.gesture = Some(Gesture::Edge { target, end });
            }
        }
    }
}

/// The grab zone inside the start or `end` side of `body`.
pub fn side(body: Rect, end: bool) -> Rect {
    let width = EDGE.min(body.width() / 3.0);
    let left = if end {
        body.right() - width
    } else {
        body.left()
    };
    Rect::from_x_y_ranges(left..=left + width, body.y_range())
}

/// White rings of the picked regions that do not follow the pointer.
fn rings(ui: &Ui, rows: &[(Row, Rect)], editor: &EditorWindow, scale: &TimeScale) {
    let moving = gesture::follows(&editor.view.gesture);
    for (index, &(row, lane)) in rows.iter().enumerate() {
        let painter = ui.painter_at(lane);
        for region in &editor.tracks.rows[index].regions {
            let picked = match (region.item, region.clip) {
                (Some(item), _) => Selected::Region(item),
                // The clips row rings its clips itself.
                (None, Some(clip)) if row != Row::Clips => Selected::Clip(clip),
                _ => continue,
            };
            if editor.view.selection.contains(picked) && !moving {
                paint::selected(&painter, paint::body(lane, scale, region.range));
            }
        }
    }
}

/// «Выбрано: N · Delete удалить · Ctrl+C копировать» at the top right of `lanes` while several
/// regions are picked (mockup 2a).
pub fn picked_tip(ui: &Ui, editor: &EditorWindow, lanes: Rect) {
    let count = editor.view.selection.len();
    if count < 2 {
        return;
    }
    let key = |k| editor.keys.label(k).unwrap_or_default();
    let line = text::picked(count, &key(EditorKey::Delete), &key(EditorKey::Copy));
    let corner = egui::pos2(lanes.right() - 12.0, lanes.top() + 2.0);
    tip(ui, corner, Align2::RIGHT_TOP, line, false);
}

/// A tip chip (mockup `.tip`) anchored at `at` by `align`, over everything; mono for times.
pub fn tip(ui: &Ui, at: egui::Pos2, align: Align2, line: String, mono: bool) {
    let painter = ui.ctx().layer_painter(LayerId::new(
        Order::Tooltip,
        ui.scope_id().with("timeline-tip"),
    ));
    let font = match mono {
        true => fonts::mono(FS_S),
        false => fonts::text(FS_S, Weight::Regular),
    };
    let galley = painter.layout_no_wrap(line, font, TEXT);
    let size = galley.size() + vec2(16.0, 8.0);
    let rect = align.anchor_size(at, size);
    painter.rect(
        rect,
        R1,
        SURFACE,
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );
    painter.galley(rect.min + vec2(8.0, 4.0), galley, TEXT);
}
