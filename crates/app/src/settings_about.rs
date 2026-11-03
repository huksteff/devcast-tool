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
    if ui
        .add(Button::new(text::OPEN_LOGS).small().icon(Icon::Folder))
        .clicked()
    {
        crate::open_logs()?;
    }
    Ok(())
}
