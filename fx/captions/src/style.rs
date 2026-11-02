//! Look of the captions (docs/porting/captions.md §5–§8, decision 000 «Субтитры») and the
//! command `captions style`.

use dc_core::{Command, CommandError, Diff, Op, Project, within};
use dc_render::{DEFAULT_FAMILY, Rgba};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::CaptionsEffect;

pub const SIZE_MIN: f64 = 16.0;
pub const SIZE_MAX: f64 = 72.0;
pub const ROWS_MAX: u32 = 4;
pub const BOTTOM_MAX: f64 = 30.0;
pub const WIDTH_MIN: f64 = 40.0;
pub const WIDTH_MAX: f64 = 95.0;
pub const RADIUS_MAX: f64 = 40.0;

/// How a page of a caption comes and goes (v1, 180 ms each way).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Animation {
    /// Shown and hidden at once.
    None,
    /// Fades in and out.
    #[default]
    Fade,
    /// Fades in rising from below, fades out.
    Rise,
    /// Fades in growing from smaller, fades out.
    Pop,
}

impl Animation {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Fade => "fade",
            Self::Rise => "rise",
            Self::Pop => "pop",
        }
    }
}

/// Look of every caption of the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CaptionsSettings {
    /// Show the captions; on by default, since there are none until `captions gen`.
    pub show: bool,
    /// Font family: Inter, Montserrat, Roboto, PT Serif, JetBrains Mono or one added to the
    /// project (`devcast font add`); a family not loaded fails the frame with a hint.
    pub font: String,
    /// Font size in px when the short side of the output is 1080 px, `SIZE_MIN..=SIZE_MAX`; it
    /// scales with the output only (decision 000).
    pub size: f64,
    /// Words already said.
    pub color: Rgba,
    /// The word being said.
    pub highlight: Rgba,
    /// Words not said yet.
    pub inactive: Rgba,
    /// Soft shadow under the letters.
    pub shadow: bool,
    /// Lines on screen at once, 1..=`ROWS_MAX`; a longer caption turns pages.
    pub rows: u32,
    /// Bottom of the plate above the frame's bottom, % of the frame height, 0..=`BOTTOM_MAX`.
    pub bottom: f64,
    /// Widest line with its padding, % of the frame width, `WIDTH_MIN..=WIDTH_MAX`.
    pub width: f64,
    /// Corner radius of the plate, px at size 30, 0..=`RADIUS_MAX`.
    pub radius: f64,
    /// Opacity of the black plate, 0..1; 0 — no plate.
    pub background: f64,
    /// How a page comes and goes.
    pub animation: Animation,
}

impl Default for CaptionsSettings {
    fn default() -> Self {
        Self {
            show: true,
            font: DEFAULT_FAMILY.to_owned(),
            size: 30.0,
            color: Rgba([0xff, 0xff, 0xff, 0xff]),
            highlight: Rgba([0xfd, 0xe0, 0x47, 0xff]),
            inactive: Rgba([0xa3, 0xa3, 0xa3, 0xff]),
            shadow: true,
            rows: 1,
            bottom: 3.0,
            width: 62.0,
            radius: 17.5,
            background: 0.9,
            animation: Animation::Fade,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptionsStyle {
    /// Show (true) or hide (false) the captions.
    pub show: Option<bool>,
    /// Inter, Montserrat, Roboto, PT Serif, JetBrains Mono or a family added to the project
    /// (`devcast font add`).
    pub font: Option<String>,
    /// Font size, px at 1080p, 16..72.
    pub size: Option<f64>,
    /// Colour of the words already said, #rrggbbaa.
    pub color: Option<Rgba>,
    /// Colour of the word being said, #rrggbbaa.
    pub highlight: Option<Rgba>,
    /// Colour of the words not said yet, #rrggbbaa.
    pub inactive: Option<Rgba>,
    /// Shadow under the letters.
    pub shadow: Option<bool>,
    /// Lines at once, 1..4; a longer caption turns pages.
    pub rows: Option<u32>,
    /// Bottom of the plate above the frame's bottom, % of the height, 0..30.
    pub bottom: Option<f64>,
    /// Widest line with padding, % of the width, 40..95.
    pub width: Option<f64>,
    /// Plate corner radius, px at size 30, 0..40.
    pub radius: Option<f64>,
    /// Plate opacity, 0..1; 0 — no plate.
    pub background: Option<f64>,
    /// none, fade, rise or pop.
    pub animation: Option<Animation>,
}

impl CaptionsStyle {
    /// Every argument as in `settings`, for `devcast defaults show`.
    pub(crate) fn of(settings: &CaptionsSettings) -> Self {
        Self {
            show: Some(settings.show),
            font: Some(settings.font.clone()),
            size: Some(settings.size),
            color: Some(settings.color),
            highlight: Some(settings.highlight),
            inactive: Some(settings.inactive),
            shadow: Some(settings.shadow),
            rows: Some(settings.rows),
            bottom: Some(settings.bottom),
            width: Some(settings.width),
            radius: Some(settings.radius),
            background: Some(settings.background),
            animation: Some(settings.animation),
        }
    }
}

impl Command for CaptionsStyle {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<CaptionsEffect>()?;
        let old = state.settings.clone();
        let new = &mut state.settings;
        new.show = self.show.unwrap_or(new.show);
        if let Some(font) = &self.font {
            new.font = family(font)?;
        }
        new.color = self.color.unwrap_or(new.color);
        new.highlight = self.highlight.unwrap_or(new.highlight);
        new.inactive = self.inactive.unwrap_or(new.inactive);
        new.shadow = self.shadow.unwrap_or(new.shadow);
        new.animation = self.animation.unwrap_or(new.animation);
        if let Some(rows) = self.rows {
            new.rows = within("строк субтитров", f64::from(rows), 1.0, f64::from(ROWS_MAX))? as u32;
        }
        let numbers = [
            (self.size, &mut new.size, "размер", SIZE_MIN, SIZE_MAX),
            (
                self.bottom,
                &mut new.bottom,
                "отступ снизу (%)",
                0.0,
                BOTTOM_MAX,
            ),
            (
                self.width,
                &mut new.width,
                "ширина (%)",
                WIDTH_MIN,
                WIDTH_MAX,
            ),
            (self.radius, &mut new.radius, "радиус", 0.0, RADIUS_MAX),
            (
                self.background,
                &mut new.background,
                "прозрачность плашки",
                0.0,
                1.0,
            ),
        ];
        for (value, field, what, min, max) in numbers {
            if let Some(value) = value {
                *field = within(&format!("{what} субтитров"), value, min, max)?;
            }
        }
        if *new == old {
            return Ok(Diff::default());
        }
        let summary = crate::style_diff::changes(&old, new);
        project.set_state::<CaptionsEffect>(&state)?;
        Ok(Diff::note(summary))
    }
}

impl Op for CaptionsStyle {
    const NAME: &'static str = "captions style";
    const ABOUT: &'static str =
        "Вид субтитров: показ, шрифт, размер, цвета слов, тень, строки, место, плашка, анимация";
    const EXAMPLE: &'static str = "captions style --size 36 --rows 2 --animation pop";
    const SETTINGS: Option<&'static str> = Some("captions");
}

fn family(font: &str) -> Result<String, CommandError> {
    let font = font.trim();
    if font.is_empty() {
        return Err(CommandError::new(
            "шрифт не указан: Inter, Montserrat, Roboto, PT Serif, JetBrains Mono или шрифт проекта (devcast font ls)",
        ));
    }
    Ok(font.to_owned())
}
