//! Click dots on the «Курсор» row (7.15, mockup `.cdot`): a 7 px dot of the row's colour at each
//! dot the cursor's lane places (`lanes::cursor::dots`, from the presses of `editor/presses.rs`);
//! a hidden one is an empty ring. In «Выбор» a click on a dot picks it (`Selected::Click`, Ctrl
//! adds or takes it out) and opens its panel (the cursor's click panel, `inspector/object.rs`).
//! A dot is grabbed over the whole row height and `GRAB` to each side, the nearest one where
//! zones meet; over a grab zone the add ghost of the row does not show (`ghost.rs`).

use dc_core::OutputTime;
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{self, BG, TEXT_3, WHITE};
use eframe::egui::{self, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, WidgetInfo, pos2};

use super::marks;
use super::row::Row;
use super::scale::{TimeScale, timecode};
use crate::editor::EditorWindow;
use crate::editor::lanes::cursor::Dot;
use crate::editor::selection::Selected;
use crate::editor::view::Tool;

/// Radius of a dot (mockup: 7 px), of a hovered one, width of the ring of a hidden one (1.5)
/// and of a picked one; half the width of the grab zone, points.
const RADIUS: f32 = 3.5;
const HOVERED: f32 = 5.0;
const HIDDEN_RING: f32 = 1.5;
const PICKED_RING: f32 = 2.0;
const GRAB: f32 = 8.0;

/// The dots on the «Курсор» row of `rows` (each with its lane): picking them and their drawing
/// over the callout regions.
pub fn ui(ui: &Ui, rows: &[(Row, Rect)], editor: &mut EditorWindow, scale: &TimeScale) {
    let Some(&(_, lane)) = rows.iter().find(|(row, _)| *row == Row::CURSOR) else {
        return;
    };
    let select = editor.view.tool == Tool::Select;
    let ctrl = ui.input(|i| i.modifiers.command);
    let painter = ui.painter_at(lane);
    let dots = editor.tracks.clicks.clone();
    for (dot, zone) in dots.iter().zip(zones(lane, &dots, scale)) {
        let Some(zone) = zone else { continue };
        let at = dot.press.at;
        let centre = pos2(scale.x(at), lane.center().y);
        let picked = Selected::Click(at);
        // The blade cuts where it is clicked: the dots only show then.
        let mut hovered = false;
        if select {
            let response = grab_zone(ui, zone, at);
            hovered = response.hovered();
            marks::pick(editor, &response, picked, ctrl);
        }
        let look = Look {
            picked: editor.view.selection.contains(picked),
            hovered,
        };
        paint(&painter, centre, dot, look);
    }
}

/// True when `pointer` is over the grab zone of a dot of the «Курсор» row `lane`.
pub fn grabbed(editor: &EditorWindow, lane: Rect, scale: &TimeScale, pointer: Pos2) -> bool {
    zones(lane, &editor.tracks.clicks, scale)
        .into_iter()
        .flatten()
        .any(|zone| zone.contains(pointer))
}

/// The grab zone of each of `dots` (in time order) on `lane`: the row high, `GRAB` to each side
/// and halfway to a nearer neighbour; none for a dot off the lane.
fn zones(lane: Rect, dots: &[Dot], scale: &TimeScale) -> Vec<Option<Rect>> {
    let xs: Vec<f32> = dots.iter().map(|d| scale.x(d.press.at)).collect();
    xs.iter()
        .enumerate()
        .map(|(i, &x)| {
            if !lane.x_range().contains(x) {
                return None;
            }
            let before = i.checked_sub(1).and_then(|p| xs.get(p));
            let left = before.map_or(x - GRAB, |b| (x - GRAB).max((b + x) / 2.0));
            let right = xs
                .get(i + 1)
                .map_or(x + GRAB, |n| (x + GRAB).min((x + n) / 2.0));
            let zone = Rect::from_x_y_ranges(left..=right, lane.y_range()).intersect(lane);
            Some(zone)
        })
        .collect()
}

fn grab_zone(ui: &Ui, zone: Rect, at: OutputTime) -> egui::Response {
    let id = ui.scope_id().with(("click-dot", at.micros()));
    let response = ui
        .interact(zone, id, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let name = text::click_dot(&timecode(at));
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, &name));
    response
}

/// How a dot is drawn besides its own state.
struct Look {
    picked: bool,
    hovered: bool,
}

/// A dot of the row's colour, an empty ring when hidden; larger under the pointer, a white ring
/// around a picked one.
fn paint(painter: &egui::Painter, centre: Pos2, dot: &Dot, look: Look) {
    let radius = if look.hovered { HOVERED } else { RADIUS };
    if look.picked {
        painter.circle_filled(centre, radius + PICKED_RING, WHITE);
        painter.circle_filled(centre, radius, BG);
    }
    if dot.hidden {
        painter.circle_stroke(centre, radius, Stroke::new(HIDDEN_RING, TEXT_3));
    } else {
        painter.circle_filled(centre, radius, tokens::HUE_CURSOR.line);
    }
}
