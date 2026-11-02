//! «По умолчанию» of the settings window (7.30): the effect settings new projects and recordings
//! start with — the file `defaults.json` of `devcast defaults` (`dc_project::Defaults`).
//! «Сохранить» takes the settings of the project open in the editor, as
//! `devcast defaults from-project`; «Сбросить» deletes the file, as `devcast defaults reset`.

use std::path::PathBuf;

use dc_core::Project;
use dc_project::Defaults;
use dc_ui_kit::strings::settings as text;
use dc_ui_kit::tokens::{S2, TEXT_2};
use dc_ui_kit::widgets::Button;
use eframe::egui::{RichText, Ui};

use crate::error::AppError;
use crate::settings_rows::{heading, wide_field};

/// The defaults file and what it holds.
#[derive(Debug)]
pub struct DefaultsFile {
    pub path: PathBuf,
    pub current: Defaults,
}

impl DefaultsFile {
    pub fn load(path: PathBuf) -> Result<Self, AppError> {
        let current = Defaults::load(&path)?;
        Ok(Self { path, current })
    }

    /// The file of `devcast defaults` (`%APPDATA%\DevCast\defaults.json`).
    pub fn of_app() -> Result<Self, AppError> {
        Self::load(dc_project::settings::defaults_file()?)
    }

    /// «Сохранить текущие как по умолчанию».
    pub fn save_from(&mut self, project: &Project) -> Result<(), AppError> {
        let defaults = Defaults::from_project(project);
        defaults.save(&self.path)?;
        self.current = defaults;
        Ok(())
    }

    /// «Сбросить к заводским».
    pub fn reset(&mut self) -> Result<(), AppError> {
        Defaults::reset(&self.path)?;
        self.current = Defaults::factory();
        Ok(())
    }
}

/// The project open in the editor and the defaults its «Сбросить» buttons go back to.
pub struct OpenProject<'a> {
    pub name: &'a str,
    pub project: &'a Project,
    pub defaults: &'a mut Defaults,
}

/// The page; a file that could not be written is returned.
pub fn defaults(
    ui: &mut Ui,
    file: &mut DefaultsFile,
    mut open: Option<OpenProject<'_>>,
) -> Result<(), AppError> {
    heading(ui, text::DEFAULTS);
    ui.label(RichText::new(text::DEFAULTS_HINT).color(TEXT_2));
    ui.add_space(S2);
    let mut result = Ok(());
    let save = Button::new(text::SAVE).small();
    let hint = match &open {
        Some(open) => text::of_project(open.name),
        None => text::NO_PROJECT.to_owned(),
    };
    wide_field(
        ui,
        text::SAVE_DEFAULTS,
        Some(&hint),
        save.size(ui).x,
        |ui| {
            if ui.add_enabled(open.is_some(), save).clicked()
                && let Some(open) = open.as_mut()
            {
                result = file.save_from(open.project);
                *open.defaults = file.current.clone();
            }
        },
    );
    let factory = file.current == Defaults::factory();
    let reset = Button::new(text::RESET).small();
    let hint = if factory {
        text::FACTORY_NOW
    } else {
        text::OWN_NOW
    };
    wide_field(
        ui,
        text::RESET_DEFAULTS,
        Some(hint),
        reset.size(ui).x,
        |ui| {
            if ui.add_enabled(!factory, reset).clicked() {
                result = file.reset();
                if let Some(open) = open.as_mut() {
                    *open.defaults = file.current.clone();
                }
            }
        },
    );
    result
}

#[cfg(test)]
#[path = "settings_defaults_tests.rs"]
mod tests;
