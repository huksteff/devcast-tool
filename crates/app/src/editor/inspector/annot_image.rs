//! «Основное» of an image annotation (mockup 3c «Картинка»): its file and size, «Заменить…» picks
//! another picture (rfd), the editor copies it into the project and sets it as `devcast image
//! set` does (`Action::Picture`, `dc_ops::import_picture`), in the key at the playhead when
//! there is one. The picture is fitted into the box keeping its proportions; it has no opacity
//! of its own (v1), so «Основное» has no «Сбросить».

use dc_fx_image::ImageBody;
use dc_ops::Step;
use dc_ui_kit::strings::annotations as text;
use dc_ui_kit::widgets::{Button, Kind};
use eframe::egui::{Label, Ui};

use super::annotation::{Edit, Section};
use super::{Cx, rows};
use crate::editor::Action;
use crate::editor::adding::PictureFile;

impl Section for ImageBody {
    const ID: &'static str = "image";

    fn basic(&self, ui: &mut Ui, cx: &mut Cx<'_>, edit: Edit) {
        let shown = match &self.picture {
            Some(picture) => {
                let name = picture.path.rsplit('/').next().unwrap_or(&picture.path);
                text::picture(name, picture.size[0], picture.size[1])
            }
            None => text::NO_FILE.to_owned(),
        };
        rows::row(ui, text::FILE, |ui| {
            ui.add(Label::new(shown).truncate());
        });
        let button = Button::new(text::REPLACE).kind(Kind::Secondary).small();
        let replace = ui.add(button).clicked();
        if replace {
            let picked = rfd::FileDialog::new()
                .set_title(text::PICK_PICTURE)
                .add_filter(text::PICTURES, &dc_media::PICTURE_EXTENSIONS)
                .pick_file();
            if let Some(path) = picked {
                cx.actions.push(Action::Picture(PictureFile {
                    id: edit.id,
                    at: edit.at,
                    path,
                }));
            }
        }
        rows::note(ui, text::PICTURE_NOTE);
    }

    fn basic_reset(&self, _edit: Edit) -> Option<Vec<Step>> {
        None
    }
}
