//! «Основное» of an image annotation (mockup 3c «Картинка»): its file and size; «Заменить…»
//! asks the editor for another picture (`Request::Picture`): it copies the file into the
//! project and sets it as `devcast image set` does, in the key at the playhead when there is
//! one. The picture is fitted into the box keeping its proportions; it has no opacity of its own
//! (v1), so «Основное» has no «Сбросить».

use crate::ImageBody;
use dc_core::Step;
use dc_ui_kit::egui::{Label, Ui};
use dc_ui_kit::strings::annotations as text;
use dc_ui_kit::widgets::{Button, Kind};

use dc_fx_annotations::panel::{Edit, Section};
use dc_ui_kit::panel::{Cx, Request, rows};

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
        if ui.add(button).clicked() {
            let (id, at) = (edit.id.to_string(), edit.at);
            cx.requests.push(Request::Picture { id, at });
        }
        rows::note(ui, text::PICTURE_NOTE);
    }

    fn basic_reset(&self, _edit: Edit) -> Option<Vec<Step>> {
        None
    }
}

/// The kind as the editor registers it: its panel section and fields.
pub const KIND: dc_fx_annotations::panel::Entry =
    dc_fx_annotations::panel::Entry::of::<ImageBody>(dc_fx_annotations::Kind::Image);
