//! Click dots on the «Курсор» row (7.15, mockup `.cdot`): a 7 px dot of the row's colour at each
//! dot the cursor's lane places (`lanes::cursor::dots`, from the presses of `editor/presses.rs`);
//! a hidden one is an empty ring. In «Выбор» a click on a dot picks it (`Selected::Click`, Ctrl
//! adds or takes it out) and opens its panel (the cursor's click panel, `inspector/object.rs`).

use dc_core::OutputTime;
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{self, BG, TEXT_3, WHITE};
use eframe::egui::{self, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, WidgetInfo, pos2, vec2};

use super::marks;
use super::scale::{TimeScale, timecode};
use super::tracks::Row;
use crate::editor::EditorWindow;
use crate::editor::lanes::cursor::Dot;
use crate::editor::selection::Selected;
use crate::editor::view::Tool;

/// Radius of a dot (mockup: 7 px), width of the ring of a hidden one (1.5) and of a picked
/// one; half the side of the grab zone, points.
const RADIUS: f32 = 3.5;
const HIDDEN_RING: f32 = 1.5;
const PICKED_RING: f32 = 2.0;
const GRAB: f32 = 5.0;

/// The dots on the «Курсор» row of `rows` (each with its lane): picking them and their drawing
/// over the callout regions.
pub fn ui(ui: &Ui, rows: &[(Row, Rect)], editor: &mut EditorWindow, scale: &TimeScale) {
    let Some(&(_, lane)) = rows.iter().find(|(row, _)| *row == Row::Cursor) else {
        return;
    };
    let select = editor.view.tool == Tool::Select;
    let ctrl = ui.input(|i| i.modifiers.command);
    let painter = ui.painter_at(lane);
    let y = lane.center().y;
    for dot in editor.tracks.clicks.clone() {
        let at = dot.press.at;
        let centre = pos2(scale.x(at), y);
        if !lane.x_range().contains(centre.x) {
            continue;
        }
        let picked = Selected::Click(at);
        // The blade cuts where it is clicked: the dots only show then.
        if select {
            let response = grab_zone(ui, lane, centre, at);
            marks::pick(editor, &response, picked, ctrl);
        }
        paint(
            &painter,
            centre,
            &dot,
            editor.view.selection.contains(picked),
        );
    }
}

fn grab_zone(ui: &Ui, lane: Rect, centre: Pos2, at: OutputTime) -> egui::Response {
    let rect = Rect::from_center_size(centre, vec2(GRAB * 2.0, GRAB * 2.0)).intersect(lane);
    let id = ui.scope_id().with(("click-dot", at.micros()));
    let response = ui
        .interact(rect, id, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let name = text::click_dot(&timecode(at));
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, &name));
    response
}

/// A dot of the row's colour, an empty ring when hidden; a white ring around a picked one.
fn paint(painter: &egui::Painter, centre: Pos2, dot: &Dot, picked: bool) {
    if picked {
        painter.circle_filled(centre, RADIUS + PICKED_RING, WHITE);
        painter.circle_filled(centre, RADIUS, BG);
    }
    if dot.hidden {
        painter.circle_stroke(centre, RADIUS, Stroke::new(HIDDEN_RING, TEXT_3));
    } else {
        painter.circle_filled(centre, RADIUS, tokens::HUE_CURSOR.line);
    }
}
