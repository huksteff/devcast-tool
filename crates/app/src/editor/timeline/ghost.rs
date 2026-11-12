//! The ghost of quick adding (mockup 1 and 2e, Cap and v1): over an empty spot of the zooms,
//! cursor, annotation and extra sound rows a dashed region shows what a click adds there and
//! how long (`adding::slot`); the click adds it with the effect's command and picks it. On an
//! extra sound row the click asks for a sound file first, on an annotation row for the kind
//! (`kinds.rs`).

use dc_core::time::Output;
use dc_core::{OutputTime, Range};
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::FS_S;
use eframe::egui::{Align2, Pos2, Rect, Response, Shape, Stroke, Ui, pos2};

use super::paint;
use super::scale::TimeScale;
use super::tracks::Row;
use crate::editor::adding::{self, SoundFile};
use crate::editor::lanes;
use crate::editor::view::KindMenu;
use crate::editor::{Action, EditorWindow};

/// Opacity of the ghost's fill.
const FILL: f32 = 0.35;

/// The ghost of `row` under the pointer hovering its empty `lane` (`empty` — the lane's own
/// response); a click adds the region. True when it did.
pub fn ui(
    ui: &Ui,
    editor: &mut EditorWindow,
    row: Row,
    lane: Rect,
    empty: &Response,
    scale: &TimeScale,
) -> bool {
    if !adding::addable(row) || editor.view.gesture.is_some() || editor.view.lift.is_some() {
        return false;
    }
    let Some(pointer) = empty.hover_pos() else {
        return false;
    };
    // A click near a click dot picks the dot, never adds a callout event.
    if row == Row::Cursor && super::dots::grabbed(editor, lane, scale, pointer) {
        return false;
    }
    let at = scale.time(pointer.x);
    let Some(range) = adding::slot(&editor.tracks, row, at) else {
        return false;
    };
    let body = paint::body(lane, scale, range);
    paint(&ui.painter_at(lane), body, row);
    if !empty.clicked() || ui.input(|i| i.modifiers.command) {
        return false;
    }
    add(editor, row, range, pointer);
    true
}

/// Adds the region of `row` at `range`; a sound row asks for the file first, an annotation row
/// for the kind (its menu opens at `pointer`).
fn add(editor: &mut EditorWindow, row: Row, range: Range<Output>, pointer: Pos2) {
    if let Row::Annotations(track) = row {
        editor.view.kinds = Some(KindMenu {
            track,
            range,
            at: pointer,
        });
        return;
    }
    if let Row::Audio(track) = row {
        let picked = crate::dialog::file()
            .set_title(text::PICK_SOUND)
            .add_filter(text::SOUND_FILES, &dc_audio::AUDIO_EXTENSIONS)
            .pick_file();
        if let Some(path) = picked {
            let end = OutputTime::ZERO + editor.tracks.duration;
            editor.ask(Action::Sound(SoundFile {
                path,
                at: range.start(),
                track,
                room: room(editor, row, range.start(), end),
            }));
        }
        return;
    }
    if let Some(step) = adding::step(row, range) {
        editor.ask(Action::Paste(vec![step]));
    }
}

/// How long a sound starting at `at` on `row` may be: up to the next region or the end.
fn room(editor: &EditorWindow, row: Row, at: OutputTime, end: OutputTime) -> dc_core::TimeDelta {
    let next = editor
        .tracks
        .rows
        .iter()
        .filter(|t| t.row == row)
        .flat_map(|t| &t.regions)
        .map(|r| r.range.start())
        .filter(|s| *s > at)
        .fold(end, OutputTime::min);
    next.since(at)
}

/// A dashed region in the colour of `row` with «+», or the default scale on the zooms row.
fn paint(painter: &eframe::egui::Painter, body: Rect, row: Row) {
    let hue = paint::hue(row);
    painter.rect_filled(body, 4, hue.fill.gamma_multiply(FILL));
    let corners = [
        body.left_top(),
        body.right_top(),
        body.right_bottom(),
        body.left_bottom(),
        body.left_top(),
    ];
    let stroke = Stroke::new(1.0, hue.line);
    painter.extend(Shape::dashed_line(&corners, stroke, 3.0, 3.0));
    let label = match row {
        Row::Zooms => lanes::zoom::ghost(),
        _ => text::GHOST.to_owned(),
    };
    painter.text(
        pos2(body.center().x, body.center().y),
        Align2::CENTER_CENTER,
        label,
        fonts::text(FS_S, Weight::Medium),
        hue.line,
    );
}
