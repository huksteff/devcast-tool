//! `text set`: what a text annotation says and how it looks.

use dc_core::{Command, CommandError, Diff, Op, Project};
use dc_fx_annotations::{AnnotId, Annotations, Kind};
use dc_render::Rgba;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::{RADIUS_MAX, SIZE_MAX, SIZE_MIN, TEXT_MAX, TextAlign, TextBody};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextSet {
    pub id: AnnotId,
    /// The words; a line break starts a new line.
    pub text: Option<String>,
    /// Font family: Inter, Montserrat, Roboto, PT Serif, JetBrains Mono or a font from the
    /// project's assets/.
    pub font: Option<String>,
    /// Font size, px when the short side of the frame is 1080 px, 12..128.
    pub size: Option<f64>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    /// left, center or right.
    pub align: Option<TextAlign>,
    /// Colour of the letters, #rrggbbaa.
    pub color: Option<Rgba>,
    /// Plate behind each line, #rrggbbaa; alpha 00 removes the plate.
    pub background: Option<Rgba>,
    /// Corner radius of the plates, px when the short side of the frame is 1080 px, 0..64.
    pub radius: Option<f64>,
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
        let mut state = project.state::<Annotations>()?;
        let ids: Vec<AnnotId> = state.annot_ids().collect();
        let annotation = state
            .annotations
            .iter_mut()
            .find(|a| a.id == self.id)
            .ok_or_else(|| CommandError::not_found(self.id, ids))?;
        if annotation.kind != Kind::Text {
            return Err(CommandError::new(format!(
                "{} — не текстовая аннотация: text set меняет только текст (devcast ls annots покажет виды)",
                self.id
            )));
        }
        let old: TextBody = annotation.body().map_err(|e| broken(self.id, &e))?;
        let new = self.change(&old)?;
        if new == old {
            return Ok(Diff::default());
        }
        annotation.set_body(&new).map_err(|e| broken(self.id, &e))?;
        project.set_state::<Annotations>(&state)?;
        Ok(Diff::changed(self.id))
    }
}

impl Op for TextSet {
    const NAME: &'static str = "text set";
    const ABOUT: &'static str = "Изменить текстовую аннотацию: текст, шрифт, размер, начертание, выравнивание, цвета, плашку";
    const EXAMPLE: &'static str =
        "text set a3 --text \"Нажми сюда\" --font Montserrat --size 48 --background \"#000000b0\"";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

fn broken(id: AnnotId, error: &serde_json::Error) -> CommandError {
    CommandError::new(format!(
        "поля текста {id} в проекте испорчены ({error}): удали аннотацию (annot rm {id}) и создай заново"
    ))
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
            "шрифт не указан: Inter, Montserrat, Roboto, PT Serif, JetBrains Mono или шрифт из assets/ проекта",
        ));
    }
    Ok(font.to_owned())
}

fn within(what: &str, value: f64, min: f64, max: f64) -> Result<f64, CommandError> {
    if (min..=max).contains(&value) {
        return Ok(value);
    }
    Err(CommandError::new(format!(
        "{what}: нужно от {min} до {max}, а не {value}"
    )))
}
