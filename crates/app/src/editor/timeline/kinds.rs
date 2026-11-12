//! The menu of kinds a click on the ghost of an annotation row opens (SCOPE «Аннотации»): text,
//! shape, picture, drawing, blur (`lanes::annotations::KINDS`). The kind picked is added where
//! the ghost was and picked; a picture asks for its file first and comes as one undo step with
//! it; a picked drawing shows the brush bar over the preview at once. A press outside or Esc
//! closes the menu (T adds text without it).

use dc_ui_kit::strings::annotations;
use dc_ui_kit::tokens::{S1, S3};
use dc_ui_kit::widgets::{Button, Kind as ButtonKind, panel_frame};
use eframe::egui::{self, Ui};

use crate::editor::adding::NewPicture;
use crate::editor::lanes::annotations::{KINDS, Kind, added, kind_name};
use crate::editor::view::KindMenu;
use crate::editor::{Action, EditorWindow};

/// Width of the menu, points.
const WIDTH: f32 = 160.0;

/// The open menu of kinds where the ghost was clicked.
pub fn menu(ui: &Ui, editor: &mut EditorWindow) {
    let Some(open) = editor.view.kinds else {
        return;
    };
    let mut chosen = None;
    let area = egui::Area::new(ui.scope_id().with("timeline-kind-menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(open.at)
        .show(ui.ctx(), |ui| {
            panel_frame().show(ui, |ui| {
                ui.set_width(WIDTH - 2.0 * S3);
                ui.spacing_mut().item_spacing.y = S1;
                for kind in KINDS {
                    let item = Button::new(kind_name(kind)).kind(ButtonKind::Ghost).small();
                    if ui.add(item).clicked() {
                        chosen = Some(kind);
                    }
                }
            });
        });
    let outside = ui.input(|i| {
        let p = &i.pointer;
        p.any_pressed()
            && p.interact_pos()
                .is_some_and(|at| !area.response.rect.contains(at))
    });
    if outside || chosen.is_some() {
        editor.view.kinds = None;
    }
    if let Some(kind) = chosen {
        add(editor, open, kind);
    }
}

/// Adds a `kind` annotation where the menu `open` was opened; a picture asks for its file.
fn add(editor: &mut EditorWindow, open: KindMenu, kind: Kind) {
    let step = added(open.range, open.track, kind);
    if kind != Kind::Image {
        editor.ask(Action::Paste(vec![step]));
        return;
    }
    let picked = crate::dialog::file()
        .set_title(annotations::PICK_PICTURE)
        .add_filter(annotations::PICTURES, &dc_media::PICTURE_EXTENSIONS)
        .pick_file();
    if let Some(path) = picked {
        editor.ask(Action::NewPicture(NewPicture { add: step, path }));
    }
}
