//! The shortcut help (mockup section 7): F1 or «?» in the header opens it over the editor. It
//! is built from the shortcut table with the user's shortcuts, grouped, searchable by name or
//! key. «Изменить в настройках» opens the settings on
//! «Горячие клавиши».

use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::{hotkeys, keys as text};
use dc_ui_kit::tokens::{FS_L, FS_S, S1, S2, S3, S4, TEXT, TEXT_2, TEXT_3};
use dc_ui_kit::widgets::{Button, IconButton, Kind, panel_frame, search_field};
use eframe::egui::{self, Align, Layout, RichText, Sense, Ui, WidgetInfo, vec2};

use crate::keymap::{EditorKey, EditorKeys, Group, Label, Title};
use crate::shortcut::Caps;

/// Width of the window, height of a row, most height of the list and the height of the rest of
/// the window with a margin to the editor's edges, points (mockup: 560 wide).
const WIDTH: f32 = 560.0;
const ROW: f32 = 26.0;
const LIST: f32 = 520.0;
const CHROME: f32 = 200.0;

/// The help while it is open.
#[derive(Debug, Default)]
pub struct Help {
    pub search: String,
}

/// What the help asks for after a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Answer {
    pub close: bool,
    pub settings: bool,
}

/// Groups of the left and the right column.
const COLUMNS: [&[Group]; 2] = [
    &[Group::Playback, Group::Montage, Group::Project],
    &[Group::Effects, Group::Edit],
];

pub fn show(ui: &Ui, help: &mut Help, keys: &EditorKeys) -> Answer {
    let modal = egui::Modal::new(egui::Id::unique("editor-help"))
        .frame(panel_frame())
        .show(ui.ctx(), |ui| {
            ui.set_width(WIDTH - 2.0 * S3);
            let answer = header(ui);
            ui.add_space(S2);
            search_field(ui, &mut help.search, text::SEARCH, ui.available_width());
            ui.add_space(S2);
            let search = help.search.to_lowercase();
            let found = |key: &EditorKey| {
                let label = keys.label(*key).unwrap_or_default();
                [key.title(), label.as_str()]
                    .iter()
                    .any(|t| t.to_lowercase().contains(&search))
            };
            let any = EditorKey::ALL.iter().any(found);
            let room = ui.ctx().content_rect().height() - CHROME;
            let list = room.clamp(4.0 * ROW, LIST);
            egui::ScrollArea::vertical()
                .max_height(list)
                .min_scrolled_height(list)
                .show(ui, |ui| {
                    if !any {
                        ui.label(RichText::new(text::NOTHING_FOUND).color(TEXT_3));
                    }
                    ui.columns(2, |columns| {
                        for (ui, groups) in columns.iter_mut().zip(COLUMNS) {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for group in groups {
                                let shown: Vec<EditorKey> = EditorKey::ALL
                                    .into_iter()
                                    .filter(|k| k.group() == *group && found(k))
                                    .collect();
                                section(ui, *group, &shown, keys);
                            }
                        }
                    });
                });
            ui.add_space(S2);
            ui.label(RichText::new(text::FOOTER).size(FS_S).color(TEXT_3));
            answer
        });
    let mut answer = modal.inner;
    answer.close |= modal.should_close() || answer.settings;
    answer
}

/// «Горячие клавиши», «Изменить в настройках» and ✕.
fn header(ui: &mut Ui) -> Answer {
    let mut answer = Answer::default();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(text::TITLE)
                .font(fonts::text(FS_L, Weight::Semibold))
                .color(TEXT),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = S1;
            answer.close = ui
                .add(IconButton::new(Icon::Close, dc_ui_kit::strings::kit::CLOSE))
                .clicked();
            let settings = Button::new(text::EDIT_IN_SETTINGS)
                .kind(Kind::Ghost)
                .small();
            answer.settings = ui.add(settings).clicked();
        });
    });
    answer
}

/// A group title and its rows; nothing when the search left no rows.
fn section(ui: &mut Ui, group: Group, shown: &[EditorKey], keys: &EditorKeys) {
    if shown.is_empty() {
        return;
    }
    ui.add_space(S2);
    ui.label(
        RichText::new(group.title())
            .font(fonts::text(FS_S, Weight::Semibold))
            .color(TEXT_2),
    );
    ui.add_space(S1);
    for key in shown {
        row(ui, *key, keys);
    }
    ui.add_space(S4 - S2);
}

/// «Выбор ......... A»: the name left, the caps right.
fn row(ui: &mut Ui, key: EditorKey, keys: &EditorKeys) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::hover());
    let shortcut = keys.shortcut(key);
    let label = match shortcut {
        Some(shortcut) => format!("{}: {}", key.title(), shortcut.label()),
        None => format!("{}: {}", key.title(), hotkeys::NONE),
    };
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, &label));
    let painter = ui.painter();
    let caps_left = match shortcut {
        Some(shortcut) => {
            let caps = shortcut.caps();
            let width = crate::keycaps::width(painter, &caps);
            let left = rect.right_center() - vec2(width + 1.0, 0.0);
            crate::keycaps::paint(painter, left, &caps);
            left.x
        }
        None => {
            let none = painter.text(
                rect.right_center(),
                egui::Align2::RIGHT_CENTER,
                hotkeys::NONE,
                fonts::text(FS_S, Weight::Regular),
                TEXT_3,
            );
            none.left()
        }
    };
    let mut title = rect;
    title.set_right(caps_left - S2);
    painter.with_clip_rect(title).text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        key.title(),
        fonts::text(FS_S, Weight::Regular),
        TEXT,
    );
}
