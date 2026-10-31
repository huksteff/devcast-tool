//! The timeline under the preview: toolbar, ruler, a row per track with its header (148 px) and
//! its regions at output times, the playhead, the navigation bar.
//!
//! What part of the project the lanes show and how tall the rows are is the `Viewport`
//! (`viewport.rs`): the whole project until the user zooms (slider, Ctrl+wheel around the
//! pointer; «↔» fits again), the wheel and the navigation bar scroll, Shift+wheel scales the rows
//! ×0.5–3 (kept in the settings). The ruler and the playhead move the playhead (`pointer.rs`).
//! The toolbar's other commands are disabled until their tasks («+ Дорожка» and
//! «+ Аудиодорожка» 7.10, «Подобрать зумы» 7.14 — its suggestions are not a registry command
//! yet, «Создать субтитры» 7.17, own window 7.26); the magnet holds its state for 7.9. The eye of
//! the captions and keys rows toggles their `show` setting; the other track header buttons wait
//! for 7.10 (removing), 7.19 (sound), 7.20 (crop).

mod paint;
mod pointer;
mod scale;
mod tracks;
mod viewport;

#[cfg(test)]
mod ui_tests;
#[cfg(test)]
mod viewport_tests;

use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{BORDER, S2, TEXT_3};
use dc_ui_kit::widgets::{Button, IconButton, Kind, Slider};
use eframe::egui::{
    self, Align, Frame, Layout, Margin, Rect, ScrollArea, Sense, Stroke, Ui, UiBuilder, vec2,
};

pub use scale::{TimeScale, timecode};
pub use tracks::{Row, Track, Tracks};
pub use viewport::Viewport;

use serde_json::Value;
use viewport::Lanes;

use super::{Action, EditorWindow};

/// Heights of the toolbar, the ruler, the «+ Аудиодорожка» row and the navigation bar; width of
/// a track header (mockup).
const TOOLBAR: f32 = 38.0;
const RULER: f32 = 24.0;
const ADD_ROW: f32 = 28.0;
const NAV: f32 = 14.0;
pub const HEAD: f32 = 148.0;
/// Width of the time scale slider of the toolbar.
const SCALE_SLIDER: f32 = 120.0;
/// Half the width of the grip of the playhead line, points.
const GRIP: f32 = 4.0;

/// The timeline; `saved_rows` is the row height factor of the settings. Returns the factor now.
pub fn ui(ui: &mut Ui, editor: &mut EditorWindow, saved_rows: f32) -> f32 {
    let full = ui.available_rect_before_wrap();
    let area = full.with_min_y(full.top() + TOOLBAR);
    let lanes = Lanes {
        left: area.left() + HEAD,
        width: (area.width() - HEAD).max(1.0),
        duration: editor.tracks.duration,
        fps: editor.fps,
    };
    let rows = editor.view.timeline.rows(saved_rows);
    let content: f32 = editor
        .tracks
        .rows
        .iter()
        .map(|t| paint::height(t.row) * rows)
        .sum();
    let overflow = content + ADD_ROW > area.height() - RULER - NAV;
    let view = &mut editor.view.timeline;
    pointer::wheel(ui, area, &lanes, view, saved_rows, overflow);
    view.settle(&lanes);
    view.follow(&lanes, editor.view.playhead);
    egui::Panel::top("timeline-bar")
        .exact_size(TOOLBAR)
        .frame(Frame::new().inner_margin(Margin::symmetric(S2 as i8, 0)))
        .show(ui, |ui| toolbar(ui, editor, &lanes));
    ui.spacing_mut().item_spacing.y = 0.0;
    let rows = editor.view.timeline.rows(saved_rows);
    let scale = editor.view.timeline.scale(&lanes);
    let (ruler, _) = ui.allocate_exact_size(vec2(area.width(), RULER), Sense::hover());
    paint::ruler(ui.painter(), ruler, &scale);
    ScrollArea::vertical()
        .auto_shrink(false)
        .max_height((area.height() - RULER - NAV).max(0.0))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut asked = None;
            for track in &editor.tracks.rows {
                asked = asked.or(row(ui, track, &editor.tracks, &scale, rows));
            }
            if let Some(action) = asked {
                editor.ask(action);
            }
            add_row(ui);
        });
    let (nav, _) = ui.allocate_exact_size(vec2(area.width(), NAV), Sense::hover());
    pointer::nav(ui, nav, &lanes, &mut editor.view.timeline);
    playhead(ui, editor, &lanes, ruler, nav.top());
    rows
}

/// The ruler and the playhead line take the pointer; the line is painted over the rows down to
/// `bottom`.
fn playhead(ui: &mut Ui, editor: &mut EditorWindow, lanes: &Lanes, ruler: Rect, bottom: f32) {
    let on_ruler = ui
        .interact(
            ruler.with_min_x(lanes.left),
            ui.scope_id().with("timeline-ruler"),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    on_ruler.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Slider, ui.is_enabled(), text::RULER)
    });
    let x = editor.view.timeline.scale(lanes).x(editor.view.playhead);
    let shown = (lanes.left..=lanes.right()).contains(&x);
    let grip = Rect::from_x_y_ranges(
        (x - GRIP).max(lanes.left)..=(x + GRIP).min(lanes.right()).max(lanes.left),
        ruler.bottom()..=bottom,
    );
    let on_line = ui
        .interact(grip, ui.scope_id().with("timeline-playhead"), Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    on_line.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Slider, ui.is_enabled(), text::PLAYHEAD)
    });
    let handles: Vec<&egui::Response> = if shown {
        vec![&on_ruler, &on_line]
    } else {
        vec![&on_ruler]
    };
    pointer::scrub(ui, &handles, lanes, editor);
    let clip = Rect::from_x_y_ranges(lanes.left..=ruler.right(), ruler.top()..=bottom);
    let scale = editor.view.timeline.scale(lanes);
    let at = editor.view.playhead;
    paint::playhead(
        &ui.painter().with_clip_rect(clip),
        ruler,
        bottom,
        scale.x(at),
        at,
    );
}

fn toolbar(ui: &mut Ui, editor: &mut EditorWindow, lanes: &Lanes) {
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.add_enabled(
            false,
            Button::new(text::ADD_TRACK)
                .kind(Kind::Ghost)
                .small()
                .icon(Icon::Plus)
                .menu(),
        );
        let magnet = IconButton::new(Icon::Magnet, text::MAGNET).on(editor.view.magnet);
        let hint = super::keys::hint(text::MAGNET, crate::keymap::EditorKey::Snap, &editor.keys);
        if ui.add(magnet).on_hover_text(hint).clicked() {
            editor.view.magnet = !editor.view.magnet;
        }
        separator(ui);
        ui.add_enabled_ui(false, |ui| {
            ui.add(
                Button::new(text::SUGGEST_ZOOMS)
                    .kind(Kind::Ghost)
                    .small()
                    .icon(Icon::Wand),
            );
            ui.add(
                Button::new(text::MAKE_CAPTIONS)
                    .kind(Kind::Ghost)
                    .small()
                    .icon(Icon::Captions),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_enabled(false, IconButton::new(Icon::Detach, text::DETACH));
            zoom(ui, editor, lanes);
        });
    });
    let rect = ui.max_rect();
    ui.painter()
        .hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, BORDER));
}

/// «↔» and the scale slider (laid out right to left); they scale around the playhead when it is
/// in view, else around the middle of the lanes.
fn zoom(ui: &mut Ui, editor: &mut EditorWindow, lanes: &Lanes) {
    let view = &mut editor.view.timeline;
    let fit = IconButton::new(Icon::Fit, text::FIT);
    if ui.add(fit).on_hover_text(text::FIT).clicked() {
        view.fit();
    }
    let x = view.scale(lanes).x(editor.view.playhead);
    let anchor = if (lanes.left..=lanes.right()).contains(&x) {
        x
    } else {
        lanes.left + lanes.width / 2.0
    };
    let mut value = view.slider(lanes);
    let slider = Slider::new(&mut value, 0.0..=1.0, text::SCALE)
        .width(SCALE_SLIDER)
        .step(0.001)
        .bare()
        .show(ui);
    if slider.changed() {
        view.set_slider(lanes, value, anchor);
    }
    let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
    Icon::ZoomIn.paint(ui.painter(), rect, TEXT_3);
}

fn separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 24.0), Sense::hover());
    ui.painter()
        .vline(rect.center().x, rect.y_range(), Stroke::new(1.0, BORDER));
}

/// A track `rows` times its height: header with its button, lane with the regions; the action of
/// a pressed button.
fn row(
    ui: &mut Ui,
    track: &Track,
    tracks: &Tracks,
    scale: &TimeScale,
    rows: f32,
) -> Option<Action> {
    let height = paint::height(track.row) * rows;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let head = rect.with_max_x(rect.left() + HEAD);
    let lane = rect.with_min_x(head.right());
    paint::header(ui.painter(), head, track);
    let name = paint::name(track.row);
    ui.interact(head, ui.scope_id().with(("track", &name)), Sense::hover())
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
    paint::lane(&ui.painter().with_clip_rect(lane), lane, track, scale);
    if track.row == Row::Clips {
        paint::speeds(&ui.painter().with_clip_rect(lane), lane, tracks, scale);
    }
    let (icon, label, dim) = header_button(track)?;
    let side = paint::HEAD_BUTTON.min(height - 4.0);
    let at = Rect::from_center_size(
        egui::pos2(head.right() - 4.0 - side / 2.0, head.center().y),
        vec2(side, side),
    );
    let mut cell = ui.new_child(UiBuilder::new().max_rect(at));
    let button = IconButton::new(icon, label)
        .size(side, 13.0_f32.min(side - 4.0))
        .dim(dim);
    let Some(action) = toggle(track) else {
        cell.add_enabled(false, button);
        return None;
    };
    let hint = if track.off { text::SHOW } else { text::HIDE };
    let clicked = cell.add(button).on_hover_text(hint).clicked();
    clicked.then_some(action)
}

/// What the header button of `track` does now: shows or hides the captions or the pressed keys
/// (the `show` setting of the effect). The other buttons wait for 7.10, 7.19 and 7.20.
fn toggle(track: &Track) -> Option<Action> {
    let op = match track.row {
        Row::Captions => "captions style",
        Row::Keys => "keys",
        _ => return None,
    };
    Some(Action::op(op, [("show", Value::Bool(track.off))]))
}

/// The header button of `track`: icon, accessible name, faded.
fn header_button(track: &Track) -> Option<(Icon, &'static str, bool)> {
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

/// «+ Аудиодорожка» under the tracks.
fn add_row(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ADD_ROW), Sense::hover());
    let head = rect.with_max_x(rect.left() + HEAD);
    let mut cell = ui.new_child(
        UiBuilder::new()
            .max_rect(head.shrink2(vec2(4.0, 1.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    let add = Button::new(text::ADD_AUDIO)
        .kind(Kind::Ghost)
        .small()
        .icon(Icon::Plus);
    cell.add_enabled(false, add);
}
