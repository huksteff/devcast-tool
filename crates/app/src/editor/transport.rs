//! The 48 px transport under the preview: tools and keyframe switches on the left, time and
//! playback in the middle, preview volume on the right.
//!
//! The tools, the switches and the volume hold their state (see `View`); playback and the jumps
//! are disabled until 7.5, the playhead does not move until then.

use dc_ui_kit::fonts;
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::editor as text;
use dc_ui_kit::tokens::{BG, BORDER, CONTROL, FS_M, ICON, S2, TEXT, TEXT_3};
use dc_ui_kit::widgets::{IconButton, Slider};
use eframe::egui::{
    self, Align, Layout, Rect, Response, Sense, Stroke, Ui, UiBuilder, WidgetInfo, vec2,
};

use super::EditorWindow;
use super::timeline::timecode;
use super::view::Tool;

/// Diameter of the play button and width of the volume rail, points (mockup).
const PLAY: f32 = 36.0;
const VOLUME: f32 = 80.0;

pub fn ui(ui: &mut Ui, editor: &mut EditorWindow) {
    let rect = ui.max_rect();
    row(ui, rect, Layout::left_to_right(Align::Center), |ui| {
        tools(ui, editor);
    });
    row(ui, rect, Layout::right_to_left(Align::Center), |ui| {
        Slider::new(&mut editor.view.volume, 0.0..=100.0, text::VOLUME)
            .width(VOLUME)
            .bare()
            .show(ui);
        ui.add(IconButton::new(Icon::Speaker, text::VOLUME));
    });
    playback(ui, rect, editor);
}

fn row(ui: &mut Ui, rect: Rect, layout: Layout, add: impl FnOnce(&mut Ui)) {
    let mut ui = ui.new_child(UiBuilder::new().max_rect(rect).layout(layout));
    ui.spacing_mut().item_spacing.x = 2.0;
    add(&mut ui);
}

fn tools(ui: &mut Ui, editor: &mut EditorWindow) {
    let view = &mut editor.view;
    for (tool, icon, label) in [
        (Tool::Select, Icon::Pointer, text::SELECT),
        (Tool::Blade, Icon::Blade, text::BLADE),
    ] {
        let button = IconButton::new(icon, label).held(view.tool == tool);
        if ui.add(button).on_hover_text(label).clicked() {
            view.tool = tool;
        }
    }
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 24.0), Sense::hover());
    ui.painter()
        .vline(rect.center().x, rect.y_range(), Stroke::new(1.0, BORDER));
    let keys = IconButton::new(Icon::Diamond, text::AUTO_KEYS).on(view.auto_keys);
    if ui.add(keys).on_hover_text(text::AUTO_KEYS).clicked() {
        view.auto_keys = !view.auto_keys;
    }
    let markers = IconButton::new(Icon::Eye, text::MARKERS).on(view.markers);
    if ui.add(markers).on_hover_text(text::MARKERS).clicked() {
        view.markers = !view.markers;
    }
}

/// Time, jumps, play and the end time, centred in the row.
fn playback(ui: &mut Ui, rect: Rect, editor: &EditorWindow) {
    let font = fonts::mono(FS_M);
    let now = timecode(editor.view.playhead);
    let end = timecode(dc_core::OutputTime::ZERO + editor.tracks.duration);
    let painter = ui.painter();
    let now = painter.layout_no_wrap(now, font.clone(), TEXT);
    let end = painter.layout_no_wrap(end, font, TEXT_3);
    let buttons = 4.0 * (CONTROL + 2.0) + PLAY + 2.0;
    let width = now.size().x + end.size().x + 2.0 * S2 * 2.0 + buttons;
    let inner = Rect::from_center_size(rect.center(), vec2(width, rect.height()));
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(S2);
            let (time, _) = ui.allocate_exact_size(now.size(), Sense::hover());
            ui.painter().galley(time.min, now, TEXT);
            ui.add_space(S2);
            ui.add_enabled_ui(false, |ui| {
                ui.add(IconButton::new(Icon::SkipBack, text::PREV_KEY));
                ui.add(IconButton::new(Icon::Left, text::PREV_FRAME));
                play_button(ui);
                ui.add(IconButton::new(Icon::Right, text::NEXT_FRAME));
                ui.add(IconButton::new(Icon::SkipForward, text::NEXT_KEY));
            });
            ui.add_space(S2);
            let (time, _) = ui.allocate_exact_size(end.size(), Sense::hover());
            ui.painter().galley(time.min, end, TEXT_3);
        },
    );
}

/// The white round play button with a dark triangle.
fn play_button(ui: &mut Ui) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(PLAY, PLAY), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), text::PLAY));
    let painter = ui.painter();
    painter.circle_filled(rect.center(), PLAY / 2.0, TEXT);
    let icon = Rect::from_center_size(rect.center() + vec2(1.0, 0.0), vec2(ICON, ICON));
    let at = |x: f32, y: f32| icon.min + vec2(x, y) * (ICON / 24.0);
    painter.add(egui::Shape::convex_polygon(
        vec![at(7.0, 4.0), at(20.0, 12.0), at(7.0, 20.0)],
        BG,
        Stroke::NONE,
    ));
    response
}
