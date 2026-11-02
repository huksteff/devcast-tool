//! The 44 px header: library and projects, undo and redo, the project name in the middle with
//! the blue dot of unsaved changes, help, settings and export.
//!
//! Undo and redo name their step and shortcut in the tooltip, help opens the shortcut help
//! (`help.rs`). «Записи» opens and closes the library (`library/`). «Экспорт» opens the export
//! dialog, the queue button its list while the queue has jobs (`export.rs`). Disabled until its
//! task: «Проекты» (7.28).
//! Renaming by a click on the name waits for 7.28: the name is the project folder's and no
//! registry command renames it (docs/QUESTIONS.md).

use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::editor as text;
use dc_ui_kit::strings::export as export_text;
use dc_ui_kit::tokens::{ACCENT_HI, BORDER, FS_L, FS_S, S1, S2, TEXT, TEXT_3};
use dc_ui_kit::widgets::{Button, IconButton, Kind};
use eframe::egui::{self, Align, Layout, Rect, Sense, Ui, UiBuilder, pos2, vec2};

use super::{Action, EditorWindow, Way, keys};
use crate::keymap::EditorKey;

/// Side of the unsaved dot and the gap after it, points (mockup).
const DOT: f32 = 7.0;
const DOT_GAP: f32 = 6.0;

/// The header; true when the settings button is pressed.
pub fn ui(ui: &mut Ui, editor: &mut EditorWindow) -> bool {
    let rect = ui.max_rect().shrink2(vec2(S2, 0.0));
    side(ui, rect, Layout::left_to_right(Align::Center), |ui| {
        let library = Button::new(text::LIBRARY)
            .kind(Kind::Ghost)
            .small()
            .icon(Icon::Library);
        if ui.add(library).clicked() {
            let open = editor.doc.dir().root().to_path_buf();
            editor.library.toggle(&open);
        }
        ui.add_enabled(
            false,
            Button::new(text::PROJECTS)
                .kind(Kind::Ghost)
                .small()
                .icon(Icon::Project)
                .menu(),
        );
        separator(ui);
        travel(ui, editor, Way::Undo);
        travel(ui, editor, Way::Redo);
    });
    let settings = side(ui, rect, Layout::right_to_left(Align::Center), |ui| {
        let export = Button::new(text::EXPORT)
            .kind(Kind::Primary)
            .icon(Icon::Export);
        let can = editor.exports.is_some() && editor.canvas.is_some();
        if ui.add_enabled(can, export).clicked() {
            editor.export = Some(super::export::ExportDialog::default());
        }
        let jobs = editor.exports.as_ref().is_some_and(|x| !x.status().is_empty());
        if jobs {
            let queue = ui
                .add(IconButton::new(Icon::Export, export_text::QUEUE).on(editor.queue_open))
                .on_hover_text(export_text::QUEUE);
            if queue.clicked() {
                editor.queue_open = !editor.queue_open;
            }
            super::export::queue_list(ui, editor, queue.rect);
        }
        ui.add_space(S1);
        let settings = ui
            .add(IconButton::new(Icon::Gear, text::SETTINGS))
            .on_hover_text(text::SETTINGS)
            .clicked();
        let help = keys::hint(text::HELP, EditorKey::Help, &editor.keys);
        if ui
            .add(IconButton::new(Icon::Help, text::HELP))
            .on_hover_text(help)
            .clicked()
        {
            editor.toggle_help();
        }
        settings
    });
    name(ui, rect, editor);
    settings
}

/// Undo or redo: enabled when there is a step, the tooltip names it.
fn travel(ui: &mut Ui, editor: &mut EditorWindow, way: Way) {
    let (icon, label, hint, key): (_, _, fn(&str) -> String, _) = match way {
        Way::Undo => (Icon::Undo, text::UNDO, text::undo_step, EditorKey::Undo),
        Way::Redo => (Icon::Redo, text::REDO, text::redo_step, EditorKey::Redo),
    };
    let step = editor
        .doc
        .next(way)
        .map(|change| keys::hint(&hint(&change.summary), key, &editor.keys));
    let button = ui.add_enabled(step.is_some(), IconButton::new(icon, label));
    if let Some(step) = step
        && button.on_hover_text(step).clicked()
    {
        editor.ask(Action::Travel(way));
    }
}

/// A row of controls along one side of `rect`.
fn side<R>(ui: &mut Ui, rect: Rect, layout: Layout, add: impl FnOnce(&mut Ui) -> R) -> R {
    let mut ui = ui.new_child(UiBuilder::new().max_rect(rect).layout(layout));
    ui.spacing_mut().item_spacing.x = S1;
    add(&mut ui)
}

/// The 1 × 24 line between groups of the header.
fn separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 24.0), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        egui::Stroke::new(1.0, BORDER),
    );
}

/// «● name .devcast» centred in the header; the dot only with unsaved changes.
fn name(ui: &mut Ui, rect: Rect, editor: &EditorWindow) {
    let unsaved = editor.doc.unsaved();
    let painter = ui.painter();
    let title = painter.layout_no_wrap(
        editor.name.clone(),
        fonts::text(FS_L, Weight::Semibold),
        TEXT,
    );
    let extension = painter.layout_no_wrap(
        text::EXTENSION.to_owned(),
        fonts::text(FS_S, Weight::Regular),
        TEXT_3,
    );
    let dot = if unsaved { DOT + DOT_GAP } else { 0.0 };
    let width = dot + title.size().x + DOT_GAP + extension.size().x;
    let left = rect.center().x - width / 2.0;
    let y = rect.center().y;
    if unsaved {
        let center = pos2(left + DOT / 2.0, y);
        painter.circle_filled(center, DOT / 2.0, ACCENT_HI);
        let spot = Rect::from_center_size(center, vec2(DOT + 4.0, DOT + 4.0));
        ui.interact(spot, ui.scope_id().with("unsaved"), Sense::hover())
            .on_hover_text(text::UNSAVED);
    }
    let title_rect = Rect::from_min_size(pos2(left + dot, y - title.size().y / 2.0), title.size());
    let extension_x = title_rect.right() + DOT_GAP;
    let baseline = title_rect.bottom() - extension.size().y - 1.0;
    let painter = ui.painter();
    painter.galley(pos2(extension_x, baseline), extension, TEXT_3);
    painter.galley(title_rect.min, title, TEXT);
    let response = ui.interact(title_rect, ui.scope_id().with("name"), Sense::hover());
    let label = editor.name.clone();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &label));
}
