//! «О программе» of the settings window: name, version and the feedback contacts of
//! `crates/update/feedback.json` with «Открыть папку логов» (8.4, mockup of stage 8).

use dc_ui_kit::Icon;
use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::settings_rows::heading;
use dc_ui_kit::strings::settings as text;
use dc_ui_kit::tokens::{FS_M, S2, S4, TEXT, TEXT_2};
use dc_ui_kit::widgets::Button;
use eframe::egui::{RichText, Ui};

use crate::error::AppError;

pub fn about(ui: &mut Ui) -> Result<(), AppError> {
    heading(ui, text::ABOUT);
    ui.label(
        RichText::new(crate::APP_NAME)
            .font(fonts::text(FS_M, Weight::Semibold))
            .color(TEXT),
    );
    ui.label(RichText::new(text::ABOUT_TEXT).color(TEXT_2));
    ui.label(RichText::new(text::version(env!("CARGO_PKG_VERSION"))).color(TEXT_2));
    ui.add_space(S4);
    heading(ui, text::FEEDBACK);
    let feedback = dc_update::feedback()?;
    ui.label(RichText::new(text::FEEDBACK_TEXT).color(TEXT));
    ui.add_space(S2);
    for contact in &feedback.contacts {
        let line = text::contact(&contact.name, &contact.value);
        match &contact.url {
            Some(url) => ui.hyperlink_to(line, url),
            None => ui.label(RichText::new(line).color(TEXT)),
        };
    }
    let repository = text::contact(text::REPOSITORY, &feedback.repository);
    ui.hyperlink_to(repository, &feedback.repository);
    ui.add_space(S2);
    authors(ui, &feedback.authors);
    if ui
        .add(Button::new(text::OPEN_LOGS).small().icon(Icon::Folder))
        .clicked()
    {
        crate::open_logs()?;
    }
    Ok(())
}

/// «Авторы», as v1 showed them: the name and the tag; a click copies the tag.
fn authors(ui: &mut Ui, authors: &[dc_update::Author]) {
    if authors.is_empty() {
        return;
    }
    ui.add_space(S2);
    heading(ui, text::AUTHORS);
    for author in authors {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&author.name).color(TEXT));
            let tag = Button::new(&author.tag).small();
            let response = ui.add(tag).on_hover_text(text::COPY_TAG);
            let copied = ui.scope_id().with(("tag-copied", &author.tag));
            if response.clicked() {
                ui.ctx().copy_text(author.tag.clone());
                let now = ui.input(|i| i.time);
                ui.data_mut(|d| d.insert_temp(copied, now));
            }
            let since = ui.data(|d| d.get_temp::<f64>(copied));
            if since.is_some_and(|t| ui.input(|i| i.time) - t < TAG_SHOWN) {
                ui.label(RichText::new(text::TAG_COPIED).color(TEXT_2));
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(250));
            }
        });
    }
    ui.add_space(S2);
}

/// How long «Тег скопирован» stays, s.
const TAG_SHOWN: f64 = 2.0;
