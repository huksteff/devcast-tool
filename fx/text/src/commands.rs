//! `text set`: what a text annotation says and how it looks.

use dc_core::time::Output;
use dc_core::{Command, CommandError, Diff, Op, Project, TimeArg, within};
use dc_fx_annotations::{AnnotId, Kind, edit_kind};
use dc_render::Rgba;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::{RADIUS_MAX, SIZE_MAX, SIZE_MIN, TEXT_MAX, TextAlign, TextBody};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextSet {
    /// Text annotation to change.
    pub id: AnnotId,
    /// The words; a line break starts a new line.
    pub text: Option<String>,
    /// Font family: Inter, Montserrat, Roboto, PT Serif, JetBrains Mono or one added to the
    /// project (`devcast font add`).
    pub font: Option<String>,
    /// Font size, px when the short side of the frame is 1080 px, 12..128.
    pub size: Option<f64>,
    /// Bold letters.
    pub bold: Option<bool>,
    /// Italic letters.
    pub italic: Option<bool>,
    /// Underlined lines.
    pub underline: Option<bool>,
    /// left, center or right.
    pub align: Option<TextAlign>,
    /// Colour of the letters, #rrggbbaa.
    pub color: Option<Rgba>,
    /// Plate behind each line, #rrggbbaa; alpha 00 removes the plate.
    pub background: Option<Rgba>,
    /// Corner radius of the plates, px when the short side of the frame is 1080 px, 0..64.
    pub radius: Option<f64>,
    /// Moment on the output video: the fields change in the key there (created with what is
    /// shown). Without it they change in the annotation and in every key.
    pub at: Option<TimeArg<Output>>,
}

impl TextSet {
    /// `old` with the given fields replaced, each checked.
    fn change(&self, old: &TextBody) -> Result<TextBody, CommandError> {
        let mut new = old.clone();
        if let Some(text) = &self.text {
            new.text = check_text(text)?;
        }
        if let Some(font) = &self.font {
            new.font = check_font(font)?;
        }
        if let Some(size) = self.size {
            new.size = within("размер шрифта", size, SIZE_MIN, SIZE_MAX)?;
        }
        if let Some(radius) = self.radius {
            new.radius = within("радиус плашки", radius, 0.0, RADIUS_MAX)?;
        }
        new.bold = self.bold.unwrap_or(new.bold);
        new.italic = self.italic.unwrap_or(new.italic);
        new.underline = self.underline.unwrap_or(new.underline);
        new.align = self.align.unwrap_or(new.align);
        new.color = self.color.unwrap_or(new.color);
        new.background = self.background.unwrap_or(new.background);
        Ok(new)
    }
}

impl Command for TextSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let wrong =
            "не текстовая аннотация: text set меняет только текст (devcast ls annots покажет виды)";
        let at = self.at.map(|TimeArg(t)| t);
        edit_kind(project, self.id, Kind::Text, wrong, at, |old: &TextBody| {
            self.change(old)
        })
    }
}

impl Op for TextSet {
    const NAME: &'static str = "text set";
    const ABOUT: &'static str = "Изменить текстовую аннотацию: текст, шрифт, размер, начертание, выравнивание, цвета, плашку (с --at — в ключе)";
    const EXAMPLE: &'static str =
        "text set a3 --text \"Нажми сюда\" --font Montserrat --size 48 --background \"#000000b0\"";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

fn check_text(text: &str) -> Result<String, CommandError> {
    if text.chars().count() > TEXT_MAX {
        return Err(CommandError::new(format!(
            "текст аннотации длиннее {TEXT_MAX} символов: раздели его на несколько аннотаций"
        )));
    }
    if text.chars().any(|c| c.is_control() && c != '\n') {
        return Err(CommandError::new(
            "в тексте аннотации есть управляющие символы (табуляция и т. п.): оставь только буквы, пробелы и переводы строки",
        ));
    }
    Ok(text.to_owned())
}

fn check_font(font: &str) -> Result<String, CommandError> {
    let font = font.trim();
    if font.is_empty() {
        return Err(CommandError::new(
            "шрифт не указан: Inter, Montserrat, Roboto, PT Serif, JetBrains Mono или шрифт проекта (devcast font ls)",
        ));
    }
    Ok(font.to_owned())
}
