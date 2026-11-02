//! The timeline under the preview: toolbar, ruler, a row per track with its header (148 px) and
//! its regions at output times, the playhead, the navigation bar.
//!
//! What part of the project the lanes show and how tall the rows are is the `Viewport`
//! (`viewport.rs`): the whole project until the user zooms (slider, Ctrl+wheel around the
//! pointer; «↔» fits again), the wheel and the navigation bar scroll, Shift+wheel scales the rows
//! ×0.5–3 (kept in the settings). The ruler and the playhead move the playhead (`pointer.rs`).
//! «+ Дорожка ▾» adds an empty annotation or extra sound row; «Подобрать зумы» adds the zooms of the
//! clicks (`devcast zoom add --at clicks`), «Создать субтитры» recognizes the speech (7.17,
//! greyed while it runs); «↗» moves the timeline into its own window (`window_bar.rs` is its
//! toolbar, `editor/timeline_window.rs` the window); the magnet makes the blade and the
//! regions stick. The clips row is `clips.rs` (pick, drag to another place, the blade), the
//! picked range of the ruler `range.rs` (Shift+drag, its menu and speed popover), the effect
//! regions `regions.rs` with the ghost of quick adding `ghost.rs`, the track headers `heads.rs`
//! (eye, mute, removing a row, the layer order of annotation rows, the crop dialog of the
//! clips), the waves of the sound rows `wave.rs` (peaks from `editor/waves.rs`).

mod clips;
mod dots;
mod gesture;
mod ghost;
mod heads;
mod key_tip;
mod marks;
mod paint;
mod places;
mod pointer;
mod range;
mod regions;
mod scale;
mod shortcuts;
mod snap;
mod tracks;
mod viewport;
mod wave;
mod window_bar;

#[cfg(test)]
mod clips_tests;
#[cfg(test)]
mod keyframes_tests;
#[cfg(test)]
mod marks_tests;
#[cfg(test)]
mod regions_tests;
#[cfg(test)]
mod rows_tests;
#[cfg(test)]
mod snap_tests;
#[cfg(test)]
mod ui_tests;
#[cfg(test)]
mod viewport_tests;
#[cfg(test)]
mod wave_tests;

use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::tokens::{BORDER, S2, TEXT_3};
use dc_ui_kit::widgets::{Button, IconButton, Kind, Slider};
use eframe::egui::{self, Align, Frame, Layout, Margin, Rect, ScrollArea, Sense, Stroke, Ui, vec2};

pub use dots::{Dot, placed as click_dots};
pub use paint::{hue, name};
pub use scale::{TimeScale, timecode};
pub use shortcuts::Sample;
pub use tracks::{Least, Region, Row, Track, Tracks};
pub use viewport::Viewport;
pub use wave::Source;

use viewport::Lanes;

use super::view::Tool;
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

/// Where the timeline is drawn: under the preview or in its own window (7.26).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Docked,
    Window,
}

/// The timeline at `place`; `saved_rows` is the row height factor of the settings. Returns the
/// factor now.
pub fn ui(ui: &mut Ui, editor: &mut EditorWindow, saved_rows: f32, place: Place) -> f32 {
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
        .show(ui, |ui| match place {
            Place::Docked => toolbar(ui, editor, &lanes),
            Place::Window => window_bar::ui(ui, editor, &lanes),
        });
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
            let mut shown = Vec::new();
            let mut headers = Vec::new();
            for track in &editor.tracks.rows {
                let shows = (&editor.tracks, &editor.waves);
                let (press, head, lane, lifted) = row(ui, track, shows, &scale, rows);
                asked = asked.or(press);
                if let (true, Row::Annotations(from)) = (lifted, track.row) {
                    editor.view.lift = Some(from);
                }
                shown.push((track.row, lane));
                headers.push((track.row, head));
            }
            if let Some(press) = asked {
                heads::press(editor, press);
            }
            if let Some(&(_, lane)) = shown.iter().find(|(row, _)| *row == Row::Clips) {
                clips::ui(ui, lane, editor, &scale, &lanes);
            }
            regions::ui(ui, &shown, editor, &scale, &lanes);
            marks::ui(ui, &shown, editor, &scale);
            dots::ui(ui, &shown, editor, &scale);
            heads::lift(ui, editor, &headers);
            heads::question(ui, editor, &headers);
            heads::add_row(ui, editor);
        });
    let below_ruler = area.with_min_y(area.top() + RULER).with_min_x(lanes.left);
    regions::picked_tip(ui, editor, below_ruler);
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
    // The blade cuts at the playhead: its line must not take that click.
    let on_line = (editor.view.tool == Tool::Select).then(|| {
        let line = ui
            .interact(grip, ui.scope_id().with("timeline-playhead"), Sense::drag())
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        line.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Slider, ui.is_enabled(), text::PLAYHEAD)
        });
        line
    });
    let ranging = range::ruler(ui, &on_ruler, lanes, editor);
    let mut handles: Vec<&egui::Response> = Vec::new();
    if !ranging {
        handles.push(&on_ruler);
    }
    if let (true, Some(line)) = (shown, &on_line) {
        handles.push(line);
    }
    pointer::scrub(ui, &handles, lanes, editor);
    range::paint(ui, editor, lanes, ruler, bottom);
    range::menu(ui, editor, lanes, ruler);
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
        add_menu(ui, editor);
        let magnet = IconButton::new(Icon::Magnet, text::MAGNET).on(editor.view.magnet);
        let hint = super::keys::hint(text::MAGNET, crate::keymap::EditorKey::Snap, &editor.keys);
        if ui.add(magnet).on_hover_text(hint).clicked() {
            editor.view.magnet = !editor.view.magnet;
        }
        separator(ui);
        let suggest = Button::new(text::SUGGEST_ZOOMS)
            .kind(Kind::Ghost)
            .small()
            .icon(Icon::Wand);
        if ui.add(suggest).clicked() {
            editor.actions.push(Action::SuggestZooms);
        }
        let idle = editor.captions.phase() == crate::editor::generation::Phase::Idle;
        let make = Button::new(text::MAKE_CAPTIONS)
            .kind(Kind::Ghost)
            .small()
            .icon(Icon::Captions);
        if ui.add_enabled(idle, make).clicked() {
            editor.actions.push(Action::Generate);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let detach = IconButton::new(Icon::Detach, text::DETACH);
            if ui.add(detach).on_hover_text(text::DETACH).clicked() {
                editor.actions.push(Action::Detach(true));
            }
            zoom(ui, editor, lanes);
        });
    });
    underline(ui);
}

/// The border under a toolbar.
fn underline(ui: &Ui) {
    let rect = ui.max_rect();
    ui.painter()
        .hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, BORDER));
}

/// «+ Дорожка ▾»: its menu adds an empty annotation row or an empty extra sound row.
fn add_menu(ui: &mut Ui, editor: &mut EditorWindow) {
    let button = Button::new(text::ADD_TRACK)
        .kind(Kind::Ghost)
        .small()
        .icon(Icon::Plus)
        .menu();
    let response = ui.add(button);
    egui::Popup::menu(&response).show(|ui| {
        let item = |label| Button::new(label).kind(Kind::Ghost).small();
        if ui
            .add(item(text::ADD_ANNOTATIONS_ROW).icon(Icon::Text))
            .clicked()
        {
            heads::add_annotation_row(editor);
        }
        if ui
            .add(item(text::ADD_AUDIO_ROW).icon(Icon::Speaker))
            .clicked()
        {
            heads::add_sound_row(editor);
        }
    });
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

/// A track `rows` times its height: header with its button, lane with the regions; what the
/// header asked, the header and the lane, whether its header began a drag.
fn row(
    ui: &mut Ui,
    track: &Track,
    (tracks, waves): (&Tracks, &crate::editor::waves::Waves),
    scale: &TimeScale,
    rows: f32,
) -> (Option<heads::Press>, Rect, Rect, bool) {
    let height = paint::height(track.row) * rows;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let head = rect.with_max_x(rect.left() + HEAD);
    let lane = rect.with_min_x(head.right());
    paint::lane(
        &ui.painter().with_clip_rect(lane),
        lane,
        track,
        scale,
        waves,
    );
    if track.row == Row::Clips {
        paint::speeds(&ui.painter().with_clip_rect(lane), lane, tracks, scale);
    }
    let (press, lifted) = heads::header(ui, head, track, height);
    (press, head, lane, lifted)
}
