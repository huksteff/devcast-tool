//! The 48 px transport under the preview: tools and keyframe switches on the left, time and
//! playback in the middle, preview volume on the right.
//!
//! The tools and the switches hold their state (see `View`); the volume scales the sound of
//! playback; play and the jumps queue a `Transport` action. Tooltips name the shortcut
//! from the user's table (`keys::hint`).

use dc_ui_kit::fonts;
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::editor as text;
use dc_ui_kit::tokens::{BG, BORDER, CONTROL, FS_M, ICON, S2, TEXT, TEXT_3};
use dc_ui_kit::widgets::{IconButton, Slider};
use eframe::egui::{
    self, Align, Layout, Rect, Response, Sense, Stroke, Ui, UiBuilder, WidgetInfo, vec2,
};

use super::timeline::timecode;
use super::view::Tool;
use super::{Action, EditorWindow, Transport, keys};
use crate::keymap::EditorKey;

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
    playback(ui, rect, editor, true);
}

fn row(ui: &mut Ui, rect: Rect, layout: Layout, add: impl FnOnce(&mut Ui)) {
    let mut ui = ui.new_child(UiBuilder::new().max_rect(rect).layout(layout));
    ui.spacing_mut().item_spacing.x = 2.0;
    add(&mut ui);
}

fn tools(ui: &mut Ui, editor: &mut EditorWindow) {
    tool_buttons(ui, editor);
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 24.0), Sense::hover());
    ui.painter()
        .vline(rect.center().x, rect.y_range(), Stroke::new(1.0, BORDER));
    auto_keys(ui, editor);
    let view = &mut editor.view;
    let markers = IconButton::new(Icon::Eye, text::MARKERS).on(view.markers);
    if ui.add(markers).on_hover_text(text::MARKERS).clicked() {
        view.markers = !view.markers;
    }
}

/// «Выбор» and «Лезвие».
pub(super) fn tool_buttons(ui: &mut Ui, editor: &mut EditorWindow) {
    for (tool, icon, label, key) in [
        (Tool::Select, Icon::Pointer, text::SELECT, EditorKey::Select),
        (Tool::Blade, Icon::Blade, text::BLADE, EditorKey::Blade),
    ] {
        let button = IconButton::new(icon, label).held(editor.view.tool == tool);
        let hint = keys::hint(label, key, &editor.keys);
        if ui.add(button).on_hover_text(hint).clicked() {
            editor.view.tool = tool;
        }
    }
}

/// ◆ «Авто-ключи».
pub(super) fn auto_keys(ui: &mut Ui, editor: &mut EditorWindow) {
    let view = &mut editor.view;
    let keys = IconButton::new(Icon::Diamond, text::AUTO_KEYS).on(view.auto_keys);
    if ui.add(keys).on_hover_text(text::AUTO_KEYS).clicked() {
        view.auto_keys = !view.auto_keys;
    }
}

/// Time, jumps, play and the end time, centred in `rect`; `steps` adds the frame steps (the
/// timeline window has room only for the jumps, mockup 7).
pub(super) fn playback(ui: &mut Ui, rect: Rect, editor: &mut EditorWindow, steps: bool) {
    let font = fonts::mono(FS_M);
    let now = timecode(editor.view.playhead);
    let end = timecode(dc_core::OutputTime::ZERO + editor.tracks.duration);
    let painter = ui.painter();
    let now = painter.layout_no_wrap(now, font.clone(), TEXT);
    let end = painter.layout_no_wrap(end, font, TEXT_3);
    let jumps = if steps { 4.0 } else { 2.0 };
    let buttons = jumps * (CONTROL + 2.0) + PLAY + 2.0;
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
            let mut asked = None;
            for (icon, label, key, transport) in [
                (
                    Icon::SkipBack,
                    text::PREV_KEY,
                    EditorKey::PrevKey,
                    Transport::Jump(false),
                ),
                (
                    Icon::Left,
                    text::PREV_FRAME,
                    EditorKey::PrevFrame,
                    Transport::Step(-1),
                ),
            ]
            .into_iter()
            .filter(|b| steps || !matches!(b.3, Transport::Step(_)))
            {
                asked = asked.or(jump_button(ui, editor, icon, label, key, transport));
            }
            let playing = editor.playback.is_playing();
            let label = if playing { text::PAUSE } else { text::PLAY };
            let hint = keys::hint(label, EditorKey::PlayPause, &editor.keys);
            if play_button(ui, playing, label)
                .on_hover_text(hint)
                .clicked()
            {
                asked = Some(Transport::PlayPause);
            }
            for (icon, label, key, transport) in [
                (
                    Icon::Right,
                    text::NEXT_FRAME,
                    EditorKey::NextFrame,
                    Transport::Step(1),
                ),
                (
                    Icon::SkipForward,
                    text::NEXT_KEY,
                    EditorKey::NextKey,
                    Transport::Jump(true),
                ),
            ]
            .into_iter()
            .filter(|b| steps || !matches!(b.3, Transport::Step(_)))
            {
                asked = asked.or(jump_button(ui, editor, icon, label, key, transport));
            }
            if let Some(transport) = asked {
                editor.ask(Action::Transport(transport));
            }
            ui.add_space(S2);
            let (time, _) = ui.allocate_exact_size(end.size(), Sense::hover());
            ui.painter().galley(time.min, end, TEXT_3);
        },
    );
}

/// A jump or frame step button; the transport it asks for when clicked.
fn jump_button(
    ui: &mut Ui,
    editor: &EditorWindow,
    icon: Icon,
    label: &str,
    key: EditorKey,
    transport: Transport,
) -> Option<Transport> {
    let hint = keys::hint(label, key, &editor.keys);
    let clicked = ui
        .add(IconButton::new(icon, label))
        .on_hover_text(hint)
        .clicked();
    clicked.then_some(transport)
}

/// The white round play button with a dark triangle, or two bars while `playing`.
fn play_button(ui: &mut Ui, playing: bool, label: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(PLAY, PLAY), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    let painter = ui.painter();
    painter.circle_filled(rect.center(), PLAY / 2.0, TEXT);
    if playing {
        let icon = Rect::from_center_size(rect.center(), vec2(ICON, ICON));
        Icon::Pause.paint(painter, icon, BG);
        return response;
    }
    let icon = Rect::from_center_size(rect.center() + vec2(1.0, 0.0), vec2(ICON, ICON));
    let at = |x: f32, y: f32| icon.min + vec2(x, y) * (ICON / 24.0);
    painter.add(egui::Shape::convex_polygon(
        vec![at(7.0, 4.0), at(20.0, 12.0), at(7.0, 20.0)],
        BG,
        Stroke::NONE,
    ));
    response
}
