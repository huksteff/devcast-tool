//! The timeline under the preview: toolbar, ruler, a row per track with its header (148 px) and
//! its regions at output times, the playhead.
//!
//! The whole project is fitted into the width. The toolbar's commands are disabled until their
//! tasks («+ Дорожка» and «+ Аудиодорожка» 7.10, zoom and fit 7.6, «Подобрать зумы» 7.14 — its
//! suggestions are not a registry command yet, «Создать субтитры» 7.17, own window 7.26); the
//! magnet holds its state for 7.9. The eye of the captions and keys rows toggles their `show`
//! setting; the other track header buttons wait for 7.10 (removing), 7.19 (sound), 7.20 (crop).

mod paint;
mod scale;
mod tracks;

use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{BORDER, S2, TEXT_3};
use dc_ui_kit::widgets::{Button, IconButton, Kind, Slider};
use eframe::egui::{
    self, Align, Frame, Layout, Margin, Rect, ScrollArea, Sense, Stroke, Ui, UiBuilder, vec2,
};

pub use scale::{TimeScale, timecode};
pub use tracks::{Row, Track, Tracks};

use serde_json::Value;

use super::{Action, EditorWindow};

/// Heights of the toolbar, the ruler and the «+ Аудиодорожка» row; width of a track header
/// (mockup).
const TOOLBAR: f32 = 38.0;
const RULER: f32 = 24.0;
const ADD_ROW: f32 = 28.0;
pub const HEAD: f32 = 148.0;
/// Width of the time scale slider of the toolbar.
const SCALE_SLIDER: f32 = 120.0;

pub fn ui(ui: &mut Ui, editor: &mut EditorWindow) {
    egui::Panel::top("timeline-bar")
        .exact_size(TOOLBAR)
        .frame(Frame::new().inner_margin(Margin::symmetric(S2 as i8, 0)))
        .show(ui, |ui| toolbar(ui, editor));
    ui.spacing_mut().item_spacing.y = 0.0;
    let area = ui.available_rect_before_wrap();
    let scale = TimeScale::fit(
        area.left() + HEAD,
        area.width() - HEAD,
        editor.tracks.duration,
    );
    let (ruler, _) = ui.allocate_exact_size(vec2(area.width(), RULER), Sense::hover());
    paint::ruler(ui.painter(), ruler, &scale);
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut asked = None;
        for track in &editor.tracks.rows {
            asked = asked.or(row(ui, track, &editor.tracks, &scale));
        }
        if let Some(action) = asked {
            editor.ask(action);
        }
        add_row(ui);
    });
    let x = scale.x(editor.view.playhead);
    paint::playhead(
        &ui.painter().with_clip_rect(area),
        ruler,
        area.bottom(),
        x,
        editor.view.playhead,
    );
}

fn toolbar(ui: &mut Ui, editor: &mut EditorWindow) {
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
        if ui.add(magnet).on_hover_text(text::MAGNET).clicked() {
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
            ui.add_enabled_ui(false, |ui| {
                ui.add(IconButton::new(Icon::Detach, text::DETACH));
                ui.add(IconButton::new(Icon::Fit, text::FIT));
                let mut zoom = 1.0;
                Slider::new(&mut zoom, 1.0..=100.0, text::SCALE)
                    .width(SCALE_SLIDER)
                    .bare()
                    .show(ui);
                let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                Icon::ZoomIn.paint(ui.painter(), rect, TEXT_3);
            });
        });
    });
    let rect = ui.max_rect();
    ui.painter()
        .hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, BORDER));
}

fn separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 24.0), Sense::hover());
    ui.painter()
        .vline(rect.center().x, rect.y_range(), Stroke::new(1.0, BORDER));
}

/// A track: header with its button, lane with the regions; the action of a pressed button.
fn row(ui: &mut Ui, track: &Track, tracks: &Tracks, scale: &TimeScale) -> Option<Action> {
    let height = paint::height(track.row);
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
    let side = paint::HEAD_BUTTON;
    let at = Rect::from_center_size(
        egui::pos2(head.right() - 4.0 - side / 2.0, head.center().y),
        vec2(side, side),
    );
    let mut cell = ui.new_child(UiBuilder::new().max_rect(at));
    let button = IconButton::new(icon, label).size(side, 13.0).dim(dim);
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
