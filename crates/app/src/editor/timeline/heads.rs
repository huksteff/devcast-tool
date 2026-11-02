//! Track headers (mockup 1, 2e): swatch and name, the button at the right — the eye of the
//! captions and keys (their `show` setting), the sound of a recording track (`audio channel
//! --mute`), the crop dialog of the clips, × of an annotation or extra sound row. × removes an
//! empty row at once and asks first in a popover by the button when the row has regions (Ctrl+Z
//! brings it back). An annotation header drags to another annotation row: the row order is the
//! layer order.
//! «+ Аудиодорожка» under the rows adds an empty extra sound row.

use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{S1, WHITE};
use dc_ui_kit::widgets::{Answer, Button, Confirm, IconButton, Kind, panel_frame};
use eframe::egui::{self, Align, Layout, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use serde_json::Value;

use super::paint;
use super::tracks::{Least, Row, Track};
use crate::editor::{Action, EditorWindow, rows};
use crate::keymap::{EditorKey, Label};

/// Width of the remove question, points (mockup).
const QUESTION: f32 = 280.0;

/// What a header asked for.
#[derive(Debug, Clone, PartialEq)]
pub enum Press {
    Action(Action),
    /// × of this row.
    Remove(Row),
}

/// The header `head` of `track` with its button; what was pressed; whether a drag of an
/// annotation header began.
pub fn header(ui: &mut Ui, head: Rect, track: &Track, height: f32) -> (Option<Press>, bool) {
    paint::header(ui.painter(), head, track);
    let name = paint::name(track.row);
    let lifts = matches!(track.row, Row::Annotations(_));
    let sense = if lifts { Sense::drag() } else { Sense::hover() };
    let mut response = ui
        .interact(head, ui.scope_id().with(("track", &name)), sense)
        .on_hover_cursor(match lifts {
            true => egui::CursorIcon::Grab,
            false => egui::CursorIcon::Default,
        });
    if lifts {
        response = response.on_hover_text(text::LAYER_HINT);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
    (button(ui, head, track, height), response.drag_started())
}

/// The button at the right of the header `head`; what it asks when pressed.
fn button(ui: &mut Ui, head: Rect, track: &Track, height: f32) -> Option<Press> {
    let (icon, label, dim) = look(track)?;
    let side = paint::HEAD_BUTTON.min(height - 4.0);
    let at = Rect::from_center_size(
        pos2(head.right() - 4.0 - side / 2.0, head.center().y),
        vec2(side, side),
    );
    let mut cell = ui.new_child(UiBuilder::new().max_rect(at));
    let button = IconButton::new(icon, label)
        .size(side, 13.0_f32.min(side - 4.0))
        .dim(dim);
    let (press, hint) = match track.row {
        Row::Captions | Row::Keys => {
            let op = if track.row == Row::Keys {
                "keys"
            } else {
                "captions style"
            };
            let action = Action::op(op, [("show", Value::Bool(track.off))]);
            let hint = if track.off { text::SHOW } else { text::HIDE };
            (Press::Action(action), hint)
        }
        Row::Mic | Row::System => {
            let action = rows::mute(track.row, !track.off)?;
            let hint = if track.off { text::UNMUTE } else { text::MUTE };
            (Press::Action(action), hint)
        }
        Row::Annotations(_) | Row::Audio(_) => (Press::Remove(track.row), text::REMOVE_TRACK),
        Row::Clips => (Press::Action(Action::OpenCrop), text::CLIP_CROP),
        _ => {
            cell.add_enabled(false, button);
            return None;
        }
    };
    let clicked = cell.add(button).on_hover_text(hint).clicked();
    clicked.then_some(press)
}

/// The header button of `track`: icon, accessible name, faded.
fn look(track: &Track) -> Option<(Icon, &'static str, bool)> {
    let off = track.off;
    Some(match track.row {
        Row::Captions | Row::Keys if off => (Icon::EyeOff, text::HIDDEN, true),
        Row::Captions | Row::Keys => (Icon::Eye, text::SHOWN, false),
        Row::Clips => (Icon::Crop, text::CLIP_CROP, false),
        Row::Annotations(_) | Row::Audio(_) => (Icon::Close, text::REMOVE_TRACK, false),
        Row::Mic | Row::System if off => (Icon::Mute, text::MUTED, true),
        Row::Mic | Row::System => (Icon::Speaker, text::SOUND, false),
        Row::Zooms | Row::Cursor => return None,
    })
}

/// Does what a header asked: × of a row with regions opens the question, an empty row goes.
pub fn press(editor: &mut EditorWindow, press: Press) {
    match press {
        Press::Action(action) => editor.ask(action),
        Press::Remove(row) => {
            let empty = editor
                .tracks
                .rows
                .iter()
                .find(|t| t.row == row)
                .is_none_or(|t| t.regions.is_empty());
            match empty {
                true => remove(editor, row),
                false => editor.view.remove = Some(row),
            }
        }
    }
}

/// How many annotation rows and extra sound rows the timeline shows now.
pub fn shown(editor: &EditorWindow) -> Least {
    let rows = &editor.tracks.rows;
    let count = |f: fn(Row) -> bool| rows.iter().filter(|t| f(t.row)).count() as u32;
    Least {
        annotations: count(|r| matches!(r, Row::Annotations(_))),
        audio: count(|r| matches!(r, Row::Audio(_))),
    }
}

/// Removes `row` with its regions as one undo step; the rows above move one down.
fn remove(editor: &mut EditorWindow, row: Row) {
    let mut least = shown(editor);
    let project = editor.doc.project();
    let steps = match row {
        Row::Annotations(track) => {
            least.annotations = least.annotations.saturating_sub(1);
            rows::remove_annotations(project, track)
        }
        Row::Audio(track) => {
            least.audio = least.audio.saturating_sub(1);
            rows::remove_sound(project, track)
        }
        _ => return,
    };
    editor.view.selection.clear();
    editor.ask(Action::Rows(least));
    if !steps.is_empty() {
        editor.ask(Action::Apply(steps));
    }
}

/// The question by the × of the row whose removal waits; `heads` are the rows with their
/// header rects. A press outside closes it.
pub fn question(ui: &Ui, editor: &mut EditorWindow, heads: &[(Row, Rect)]) {
    let Some(row) = editor.view.remove else {
        return;
    };
    let Some(&(_, head)) = heads.iter().find(|(r, _)| *r == row) else {
        editor.view.remove = None;
        return;
    };
    let count = editor
        .tracks
        .rows
        .iter()
        .find(|t| t.row == row)
        .map_or(0, |t| t.regions.len());
    let undo = editor.keys.label(EditorKey::Undo).unwrap_or_default();
    let (title, body) = (
        text::remove_title(&paint::name(row)),
        match row {
            Row::Audio(_) => text::remove_sound(count, &undo),
            _ => text::remove_annotations(count, &undo),
        },
    );
    let mut answer = None;
    let area = egui::Area::new(ui.scope_id().with("timeline-remove-row"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(head.right() + S1, head.top()))
        .show(ui.ctx(), |ui| {
            panel_frame().show(ui, |ui| {
                ui.set_width(QUESTION);
                answer = Confirm {
                    title: &title,
                    text: &body,
                    cancel: text::REMOVE_CANCEL,
                    confirm: text::REMOVE_CONFIRM,
                    kind: Kind::Danger,
                }
                .show(ui);
            });
        });
    let outside = ui.input(|i| {
        let p = &i.pointer;
        p.any_pressed()
            && p.interact_pos()
                .is_some_and(|at| !area.response.rect.contains(at))
    });
    match answer {
        Some(Answer::Confirm) => {
            editor.view.remove = None;
            remove(editor, row);
        }
        Some(Answer::Cancel) => editor.view.remove = None,
        None if outside => editor.view.remove = None,
        None => {}
    }
}

/// The drag of an annotation header: a white line where it would go; the release moves the
/// row there. `heads` are the rows with their header rects.
pub fn lift(ui: &Ui, editor: &mut EditorWindow, heads: &[(Row, Rect)]) {
    let Some(from) = editor.view.lift else { return };
    let (down, pointer) = ui.input(|i| (i.pointer.primary_down(), i.pointer.interact_pos()));
    let annotation_rows: Vec<(u32, Rect)> = heads
        .iter()
        .filter_map(|&(row, rect)| match row {
            Row::Annotations(track) => Some((track, rect)),
            _ => None,
        })
        .collect();
    let (Some(first), Some(last), Some(at)) =
        (annotation_rows.first(), annotation_rows.last(), pointer)
    else {
        editor.view.lift = None;
        return;
    };
    let y = at.y.clamp(first.1.top(), last.1.bottom() - 1.0);
    let Some(&(to, rect)) = annotation_rows
        .iter()
        .find(|(_, r)| r.y_range().contains(y))
    else {
        return;
    };
    if !down {
        editor.view.lift = None;
        if to != from {
            let steps = rows::move_annotations(editor.doc.project(), from, to);
            editor.ask(Action::Apply(steps));
        }
        return;
    }
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        ui.scope_id().with("timeline-lift"),
    ));
    if let Some(&(_, source)) = annotation_rows.iter().find(|(t, _)| *t == from) {
        painter.rect_filled(source, 0, WHITE.gamma_multiply(0.08));
    }
    if to != from {
        let edge = if to > from { rect.bottom() } else { rect.top() };
        let x = rect.left()..=ui.max_rect().right();
        painter.hline(x, edge, Stroke::new(2.0, WHITE));
    }
}

/// «+ Аудиодорожка» under the rows: one more empty extra sound row.
pub fn add_row(ui: &mut Ui, editor: &mut EditorWindow) {
    let height = super::ADD_ROW;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let head = rect.with_max_x(rect.left() + super::HEAD);
    let mut cell = ui.new_child(
        UiBuilder::new()
            .max_rect(head.shrink2(vec2(4.0, 1.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    let add = Button::new(text::ADD_AUDIO)
        .kind(Kind::Ghost)
        .small()
        .icon(Icon::Plus);
    if cell.add(add).clicked() {
        add_sound_row(editor);
    }
}

/// One more empty extra sound row.
pub fn add_sound_row(editor: &mut EditorWindow) {
    let least = shown(editor);
    editor.ask(Action::Rows(Least {
        audio: least.audio + 1,
        ..least
    }));
}

/// One more empty annotation row.
pub fn add_annotation_row(editor: &mut EditorWindow) {
    let least = shown(editor);
    editor.ask(Action::Rows(Least {
        annotations: least.annotations + 1,
        ..least
    }));
}
