//! Key markers on the annotation regions (7.12, mockup `.kf`): a ◆ at the output time of every
//! key that shows, while «маркеры» (the eye of the transport) is on. In «Выбор» a click picks a
//! key and puts the playhead on it (Ctrl adds or takes it out, the playhead stays), a drag
//! moves it in time inside its annotation, sticking like a region edge
//! (`snap::point`); the release is `annot key move`, Delete is `annot key rm`
//! (`editor/clipboard.rs`). Hovering a marker shows what its key changes (`key_tip.rs`).

use dc_core::time::Output;
use dc_core::{OutputTime, Range, TimeMap};
use dc_fx_annotations::{AnnotId, Annotation};
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{BG, KEYFRAME, WHITE};
use eframe::egui::{
    self, Color32, CursorIcon, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, WidgetInfo,
    pos2, vec2,
};

use super::scale::{TimeScale, timecode};
use super::tracks::Row;
use super::{key_tip, paint};
use crate::editor::selection::{Item, Selected};
use crate::editor::view::{Gesture, Tool};
use crate::editor::{Action, EditorWindow, Transport};

/// Half the diagonal of a marker (mockup: a 9 px square turned 45°) and of the ring of a
/// picked one (2 more).
const HALF: f32 = 6.0;
const RING: f32 = 2.0;
/// Half the width of the grab zone of a marker, points.
const GRAB: f32 = 6.0;

/// Output times of the keys of `annotation`, which shows at `span`: a key past a trimmed edge or
/// in a cut does not show; a key at the end is at the end of the span.
pub fn shown(map: &TimeMap, annotation: &Annotation, span: Range<Output>) -> Vec<OutputTime> {
    let range = annotation.range;
    let times = annotation.keys.iter().filter_map(|key| {
        let t = range.start() + key.at;
        if t == range.end() {
            return Some(span.end());
        }
        range.contains(t).then_some(())?;
        map.locate(&annotation.media, t).map(|(_, at)| at)
    });
    times.collect()
}

/// The markers of the annotation rows of `rows` (each with its lane): picking, the start of a
/// drag, and their drawing over the regions. Nothing while the markers are hidden.
pub fn ui(ui: &Ui, rows: &[(Row, Rect)], editor: &mut EditorWindow, scale: &TimeScale) {
    if !editor.view.markers {
        return;
    }
    let select = editor.view.tool == Tool::Select;
    let ctrl = ui.input(|i| i.modifiers.command);
    let press = ui.input(|i| i.pointer.press_origin());
    let dragged = match editor.view.gesture {
        Some(Gesture::Key { id, from, .. }) => Some((id, from)),
        _ => None,
    };
    for (index, &(row, lane)) in rows.iter().enumerate() {
        if !matches!(row, Row::Annotations(_)) {
            continue;
        }
        let painter = ui.painter_at(lane);
        for region in editor.tracks.rows[index].regions.clone() {
            let Some(Item::Annotation(id)) = region.item else {
                continue;
            };
            let y = paint::body(lane, scale, region.range).center().y;
            for at in region.keys {
                let centre = pos2(scale.x(at), y);
                if !lane.x_range().contains(centre.x) {
                    continue;
                }
                let key = Selected::Key { id, at };
                // The blade cuts where it is clicked: the markers only show then.
                if select {
                    let response = grab_zone(ui, lane, centre, (id, at));
                    let response = match editor.canvas {
                        Some(canvas) => response.on_hover_ui(|ui| {
                            let project = editor.doc.project();
                            if let Some(tip) = key_tip::tip(project, canvas, id, at) {
                                key_tip::ui(ui, &tip);
                            }
                        }),
                        None => response,
                    };
                    pick(editor, &response, key, ctrl);
                    // As in DaVinci: a click puts the playhead on the key, so the inspector edits it.
                    if response.clicked() && !ctrl {
                        editor.ask(Action::Transport(Transport::Scrub(at)));
                    }
                    if response.drag_started() {
                        let grab = scale.time(press.map_or(centre.x, |p| p.x));
                        editor.view.gesture = Some(Gesture::Key {
                            id,
                            from: at,
                            grab,
                            row: index,
                        });
                    }
                }
                if dragged != Some((id, at)) {
                    diamond(&painter, centre, editor.view.selection.contains(key));
                }
            }
        }
    }
}

/// The grab zone of the marker of annotation `id` at `at`, centred at `centre`.
fn grab_zone(ui: &Ui, lane: Rect, centre: Pos2, (id, at): (AnnotId, OutputTime)) -> Response {
    let rect = Rect::from_center_size(centre, vec2(GRAB * 2.0, HALF * 2.0)).intersect(lane);
    let key = ui.scope_id().with(("key-marker", id, at.micros()));
    let response = ui
        .interact(rect, key, Sense::click_and_drag())
        .on_hover_cursor(CursorIcon::ResizeHorizontal);
    let name = text::key_marker(&timecode(at));
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, &name));
    response
}

/// A click picks `key` (Ctrl adds or takes it out); a drag of a key not picked picks it alone.
pub(super) fn pick(editor: &mut EditorWindow, response: &Response, key: Selected, ctrl: bool) {
    let selection = &mut editor.view.selection;
    if response.clicked() {
        match ctrl {
            true => selection.toggle(key),
            false => selection.only(key),
        }
    }
    if response.drag_started() && !selection.contains(key) {
        selection.only(key);
    }
}

/// A marker at `centre`: light blue with a dark edge, or white in a blue ring when `picked`.
pub fn diamond(painter: &Painter, centre: Pos2, picked: bool) {
    let shape = |half: f32, fill: Color32, edge: Stroke| {
        let points = vec![
            centre + vec2(0.0, -half),
            centre + vec2(half, 0.0),
            centre + vec2(0.0, half),
            centre + vec2(-half, 0.0),
        ];
        Shape::convex_polygon(points, fill, edge)
    };
    if picked {
        painter.add(shape(HALF + RING, KEYFRAME, Stroke::NONE));
        painter.add(shape(HALF, WHITE, Stroke::NONE));
    } else {
        painter.add(shape(HALF, KEYFRAME, Stroke::new(1.0, BG)));
    }
}
