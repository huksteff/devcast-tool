//! The library panel (mockup 6): header with «Открыть папку» and «Импорт…», search, two columns
//! of tiles (picture with the length, name, frame size; a check when picked, + under the
//! pointer), «Выбрано N · На таймлайн · В корзину» and, after the bin, «Отменить».
//!
//! A click picks one tile, Ctrl+click adds or takes away one, Shift+click picks from the last
//! clicked; a double click or + adds the tile, a drag carries the picked tiles (or this one).

use std::path::PathBuf;

use dc_core::TimeDelta;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::library as text;
use dc_ui_kit::tokens::{
    ACCENT_HI, BORDER, FS_M, FS_S, PILL, R1, R2, REC, S1, S2, S3, SURFACE, SURFACE_2, TEXT, TEXT_2,
    TEXT_3, WHITE,
};
use dc_ui_kit::widgets::{Button, IconButton, Kind, search_field};
use eframe::egui::{
    self, Align, Align2, Color32, Frame, Layout, Margin, Order, Rect, RichText, ScrollArea, Sense,
    Stroke, StrokeKind, Ui, UiBuilder, Vec2, WidgetInfo, pos2, vec2,
};

use super::{Ask, Dragged, Entry, Library};
use crate::editor::{Action, EditorWindow};

/// Width of the panel and gap between tiles, points (mockup).
const WIDTH: f32 = 300.0;
const GAP: f32 = S2;
/// Height of a tile's two lines of text under its picture.
const LINES: f32 = 36.0;
/// Heights of the «Выбрано» row and of the «Отменить» row under the tiles.
const FOOTER: f32 = 32.0;
const UNDO_ROW: f32 = 40.0;

/// The panel over the left of `area` (the preview) while it is open.
pub fn show(ui: &Ui, editor: &mut EditorWindow, area: Rect) {
    if !editor.library.open {
        return;
    }
    let rect = Rect::from_min_size(area.min, vec2(WIDTH, area.height()));
    egui::Area::new(ui.scope_id().with("library"))
        .order(Order::Middle)
        .fixed_pos(rect.min)
        .show(ui.ctx(), |ui| {
            let frame = Frame::new()
                .fill(SURFACE)
                .stroke(Stroke::new(1.0, BORDER))
                .inner_margin(Margin::same(S3 as i8));
            frame.show(ui, |ui| {
                let inner = rect.size() - Vec2::splat(2.0 * S3);
                ui.set_min_size(inner);
                ui.set_max_size(inner);
                panel(ui, editor, inner.x);
            });
        });
    ghost(ui);
}

fn panel(ui: &mut Ui, editor: &mut EditorWindow, width: f32) {
    ui.spacing_mut().item_spacing = vec2(S2, S2);
    header(ui, editor);
    search_field(ui, &mut editor.library.search, text::SEARCH, width);
    let undo = match editor.library.trashed.is_empty() {
        true => 0.0,
        false => UNDO_ROW + S2,
    };
    let footer = FOOTER + S2 + undo;
    let height = (ui.available_height() - footer).max(0.0);
    ScrollArea::vertical()
        .id_salt("library-tiles")
        .max_height(height)
        .min_scrolled_height(height)
        .auto_shrink(false)
        .show(ui, |ui| tiles(ui, editor, width));
    footer_row(ui, editor);
    if !editor.library.trashed.is_empty() {
        undo_row(ui, editor);
    }
}

fn header(ui: &mut Ui, editor: &mut EditorWindow) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(text::TITLE).size(FS_M).strong().color(TEXT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(Button::new(text::IMPORT).small()).clicked() {
                let picked = crate::dialog::file()
                    .set_title(text::PICK_VIDEOS)
                    .add_filter(text::VIDEOS, &dc_media::VIDEO_EXTENSIONS)
                    .pick_files();
                if let Some(videos) = picked {
                    editor.ask(Action::Library(Ask::Import(videos)));
                }
            }
            let folder = IconButton::new(Icon::Folder, text::OPEN_FOLDER);
            if ui.add(folder).on_hover_text(text::OPEN_FOLDER).clicked() {
                editor.ask(Action::Library(Ask::OpenFolder));
            }
        });
    });
}

fn tiles(ui: &mut Ui, editor: &mut EditorWindow, width: f32) {
    let library = &editor.library;
    let shown: Vec<Entry> = library.shown().into_iter().cloned().collect();
    if shown.is_empty() {
        let note = match library.entries.is_empty() {
            true => text::EMPTY,
            false => text::NOT_FOUND,
        };
        ui.label(RichText::new(note).size(FS_S).color(TEXT_3));
        return;
    }
    let side = (width - GAP) / 2.0;
    let size = vec2(side, side * 9.0 / 16.0 + LINES);
    for row in shown.chunks(2) {
        let (line, _) = ui.allocate_exact_size(vec2(width, size.y), Sense::hover());
        for (n, entry) in row.iter().enumerate() {
            let rect = Rect::from_min_size(line.min + vec2(n as f32 * (side + GAP), 0.0), size);
            tile(ui, editor, entry, &shown, rect);
        }
    }
}

/// One tile at `rect`; `shown` are the tiles in their order, for Shift+click.
fn tile(ui: &mut Ui, editor: &mut EditorWindow, entry: &Entry, shown: &[Entry], rect: Rect) {
    let id = ui.make_persistent_id(("library-tile", &entry.root));
    let response = ui.interact(rect, id, Sense::click_and_drag());
    let library = &mut editor.library;
    let picked = library.picked.contains(&entry.root);
    response.widget_info(|| WidgetInfo::selected(egui::Role::Button, true, picked, &entry.name));
    let picture = Rect::from_min_size(rect.min, vec2(rect.width(), rect.width() * 9.0 / 16.0));
    paint(ui, library, entry, rect, picture, picked);
    let plus = Rect::from_min_size(picture.right_top() + vec2(-26.0, 4.0), vec2(22.0, 22.0));
    let hovered = response.hovered() || ui.rect_contains_pointer(plus);
    if hovered {
        let label = text::add_one(&entry.name);
        // A child of its own: `put` would push the next row of tiles down.
        let mut cell = ui.new_child(UiBuilder::new().max_rect(plus));
        let add = cell.add(IconButton::new(Icon::Plus, &label).size(22.0, 14.0));
        if add.clicked() {
            editor.ask(Action::Library(Ask::Add(vec![entry.root.clone()])));
            return;
        }
    }
    if response.double_clicked() {
        editor.ask(Action::Library(Ask::Add(vec![entry.root.clone()])));
    } else if response.clicked() {
        let modifiers = ui.input(|i| i.modifiers);
        click(&mut editor.library, entry, shown, modifiers);
    }
    if response.drag_started() {
        let library = &editor.library;
        let roots = match library.picked.contains(&entry.root) {
            true => picked_in_order(library, shown),
            false => vec![entry.root.clone()],
        };
        egui::DragAndDrop::set_payload(ui.ctx(), Dragged(roots));
    }
}

fn click(library: &mut Library, entry: &Entry, shown: &[Entry], modifiers: egui::Modifiers) {
    let root = entry.root.clone();
    let at = |root: &PathBuf| shown.iter().position(|e| e.root == *root);
    let anchor = library.anchor.as_ref().and_then(at);
    match (modifiers.shift, modifiers.command, anchor, at(&root)) {
        (true, _, Some(from), Some(to)) => {
            library.picked = shown[from.min(to)..=from.max(to)]
                .iter()
                .map(|e| e.root.clone())
                .collect();
            return;
        }
        (_, true, _, _) => match library.picked.iter().position(|p| *p == root) {
            Some(n) => drop(library.picked.remove(n)),
            None => library.picked.push(root.clone()),
        },
        _ => library.picked = vec![root.clone()],
    }
    library.anchor = Some(root);
}

/// The picked recordings in the order the panel shows them.
fn picked_in_order(library: &Library, shown: &[Entry]) -> Vec<PathBuf> {
    shown
        .iter()
        .filter(|e| library.picked.contains(&e.root))
        .map(|e| e.root.clone())
        .collect()
}

fn paint(ui: &Ui, library: &mut Library, entry: &Entry, rect: Rect, picture: Rect, picked: bool) {
    let texture = library.picture(ui.ctx(), &entry.root);
    let painter = ui.painter();
    painter.rect_filled(picture, R1, SURFACE_2);
    if let Some(texture) = texture {
        let fitted = fit(picture, texture.size_vec2());
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture.id(), fitted, uv, Color32::WHITE);
    }
    let (border, width) = match picked {
        true => (ACCENT_HI, 2.0),
        false => (BORDER, 1.0),
    };
    painter.rect_stroke(picture, R1, Stroke::new(width, border), StrokeKind::Inside);
    let length = painter.layout_no_wrap(clock(entry.duration), fonts::mono(FS_S), WHITE);
    let badge = Align2::RIGHT_BOTTOM.anchor_size(
        picture.right_bottom() - vec2(S1, S1),
        length.size() + vec2(S2, 2.0),
    );
    painter.rect_filled(badge, R1, PILL);
    painter.galley(badge.min + vec2(S1, 1.0), length, WHITE);
    if picked {
        let check = Rect::from_min_size(picture.min + vec2(S1, S1), vec2(18.0, 18.0));
        painter.rect_filled(check, R1, ACCENT_HI);
        Icon::Check.paint(painter, check.shrink(2.0), WHITE);
    }
    let name = painter.layout(
        entry.name.clone(),
        fonts::text(FS_S, Weight::Medium),
        TEXT,
        rect.width(),
    );
    let top = picture.bottom() + S1;
    painter.galley(pos2(rect.left(), top), name, TEXT);
    let details = text::details(entry.imported, entry.size.width, entry.size.height);
    let details = painter.layout_no_wrap(details, fonts::text(FS_S, Weight::Regular), TEXT_2);
    painter.galley(pos2(rect.left(), top + 17.0), details, TEXT_2);
}

/// `size` fitted into `rect`, centred.
fn fit(rect: Rect, size: Vec2) -> Rect {
    let scale = (rect.width() / size.x).min(rect.height() / size.y);
    Rect::from_center_size(rect.center(), size * scale)
}

/// «0:52», «12:04».
fn clock(length: TimeDelta) -> String {
    let secs = length.as_secs_f64().round() as i64;
    format!("{}:{:02}", secs / 60, secs % 60)
}

fn footer_row(ui: &mut Ui, editor: &mut EditorWindow) {
    let library = &editor.library;
    let shown: Vec<Entry> = library.shown().into_iter().cloned().collect();
    let roots = picked_in_order(library, &shown);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(text::picked(roots.len()))
                .size(FS_S)
                .color(TEXT_2),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let some = !roots.is_empty();
            let bin = IconButton::new(Icon::Trash, text::TRASH).color(REC);
            if ui
                .add_enabled(some, bin)
                .on_hover_text(text::TRASH)
                .clicked()
            {
                editor.ask(Action::Library(Ask::Trash(roots.clone())));
            }
            if ui
                .add_enabled(some, Button::new(text::TO_TIMELINE).small())
                .clicked()
            {
                editor.ask(Action::Library(Ask::Add(roots.clone())));
            }
        });
    });
}

fn undo_row(ui: &mut Ui, editor: &mut EditorWindow) {
    let frame = Frame::new()
        .fill(SURFACE_2)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(R2)
        .inner_margin(Margin::symmetric(S2 as i8, S1 as i8));
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            let count = editor.library.trashed.len();
            ui.label(RichText::new(text::trashed(count)).size(FS_S).color(TEXT));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(Button::new(text::UNDO).small().kind(Kind::Ghost))
                    .clicked()
                {
                    editor.ask(Action::Library(Ask::Restore));
                }
            });
        });
    });
}

/// «2» by the pointer while recordings are dragged from the library.
fn ghost(ui: &Ui) {
    let Some(dragged) = egui::DragAndDrop::payload::<Dragged>(ui.ctx()) else {
        return;
    };
    let Some(at) = ui.ctx().pointer_latest_pos() else {
        return;
    };
    let layer = egui::LayerId::new(Order::Tooltip, ui.scope_id().with("library-drag"));
    let painter = ui.ctx().layer_painter(layer);
    let label = painter.layout_no_wrap(
        dragged.0.len().to_string(),
        fonts::text(FS_S, Weight::Semibold),
        WHITE,
    );
    let rect = Rect::from_min_size(at + vec2(12.0, 12.0), label.size() + vec2(2.0 * S2, S1));
    painter.rect_filled(rect, R1, ACCENT_HI);
    painter.galley(rect.min + vec2(S2, S1 / 2.0), label, WHITE);
}
