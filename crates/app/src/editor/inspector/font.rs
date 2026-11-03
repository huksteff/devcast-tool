//! The font row of the text annotation and of the captions: a list of the built-in families and
//! the project's own, and «Добавить шрифт…» under it, which picks a `.ttf`/`.otf` file (rfd);
//! the editor copies it into the project and chooses it (`Action::Font`).

use dc_ops::Step;
use dc_ui_kit::strings::inspector as names;
use dc_ui_kit::widgets::{Button, Kind, dropdown};
use eframe::egui::Ui;

use super::{Cx, rows};
use crate::editor::Action;
use crate::editor::fonts::FontFile;

/// The row `label` with family `now`: the family picked from the list. A file added with the
/// button is chosen by `then` with its family as `font`.
pub fn font(ui: &mut Ui, cx: &mut Cx<'_>, label: &str, now: &str, then: Step) -> Option<String> {
    let mut families: Vec<(String, &str)> = cx
        .base
        .fonts
        .iter()
        .map(|f| (f.clone(), f.as_str()))
        .collect();
    if !cx.base.fonts.iter().any(|f| f == now) {
        families.push((now.to_owned(), now));
    }
    let picked = rows::row(ui, label, |ui| {
        let mut chosen = now.to_owned();
        let width = ui.available_width();
        dropdown(ui, label, &mut chosen, &families, width)
            .changed()
            .then_some(chosen)
    });
    let button = Button::new(names::ADD_FONT).kind(Kind::Secondary).small();
    if ui.add(button).clicked() {
        let file = rfd::FileDialog::new()
            .set_title(names::PICK_FONT)
            .add_filter(names::FONT_FILES, &dc_render::FONT_EXTENSIONS)
            .pick_file();
        if let Some(path) = file {
            cx.actions.push(Action::Font(FontFile { path, then }));
        }
    }
    picked
}
