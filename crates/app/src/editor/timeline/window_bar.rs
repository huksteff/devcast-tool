//! The toolbar of the timeline window (mockup 7): the tools, the magnet, «+ Дорожка» and the
//! icon buttons of the docked toolbar plus ◆ on the left, time and playback in the middle (the
//! preview with its transport may be on another monitor), the time scale on the right.

use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::timeline as text;
use dc_ui_kit::widgets::IconButton;
use eframe::egui::{Align, Layout, Ui};

use super::viewport::Lanes;
use super::{add_menu, separator, underline, zoom};
use crate::editor::generation::Phase;
use crate::editor::transport;
use crate::editor::{Action, EditorWindow};
use crate::keymap::EditorKey;

pub fn ui(ui: &mut Ui, editor: &mut EditorWindow, lanes: &Lanes) {
    let rect = ui.max_rect();
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        transport::tool_buttons(ui, editor);
        let magnet = IconButton::new(Icon::Magnet, text::MAGNET).on(editor.view.magnet);
        let hint = crate::editor::keys::hint(text::MAGNET, EditorKey::Snap, &editor.keys);
        if ui.add(magnet).on_hover_text(hint).clicked() {
            editor.view.magnet = !editor.view.magnet;
        }
        separator(ui);
        add_menu(ui, editor);
        let suggest = IconButton::new(Icon::Wand, text::SUGGEST_ZOOMS);
        if ui.add(suggest).on_hover_text(text::SUGGEST_ZOOMS).clicked() {
            editor.actions.push(Action::SuggestZooms);
        }
        let idle = editor.captions.phase() == Phase::Idle;
        let make = IconButton::new(Icon::Captions, text::MAKE_CAPTIONS);
        if ui
            .add_enabled(idle, make)
            .on_hover_text(text::MAKE_CAPTIONS)
            .clicked()
        {
            editor.actions.push(Action::Generate);
        }
        transport::auto_keys(ui, editor);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            zoom(ui, editor, lanes);
        });
    });
    transport::playback(ui, rect, editor, false);
    underline(ui);
}
