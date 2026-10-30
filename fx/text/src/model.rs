//! The fields of a text annotation, kept in `Annotation::body` (fx/annotations).

use dc_fx_annotations::{Blend, step};
use dc_render::{Align, FAMILIES, Rgba};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Font size, px when the short side of the frame is 1080 px (SCOPE: 12–128).
pub const SIZE_MIN: f64 = 12.0;
pub const SIZE_MAX: f64 = 128.0;
/// Plate corner radius, px when the short side of the frame is 1080 px.
pub const RADIUS_MAX: f64 = 64.0;
/// Longest text, characters.
pub const TEXT_MAX: usize = 2000;

/// What a text annotation shows; a missing field takes its default (v1 defaults, the font is
/// the built-in Inter, decision 000).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TextBody {
    /// The words; a line break starts a new line, long lines wrap at the box width.
    pub text: String,
    /// Font family: Inter, Montserrat, Roboto, PT Serif, JetBrains Mono or a font file put in
    /// the project's assets/.
    pub font: String,
    /// Font size, px when the short side of the frame is 1080 px, 12..128; the zoom camera
    /// scales it with the box.
    pub size: f64,
    /// Bold letters; on by default (v1).
    pub bold: bool,
    /// Italic letters.
    pub italic: bool,
    /// Underlined lines.
    pub underline: bool,
    /// Lines to the left, centre or right of the box.
    pub align: TextAlign,
    /// Colour of the letters and of the underline.
    pub color: Rgba,
    /// Plate behind each line; alpha 00 means no plate.
    pub background: Rgba,
    /// Corner radius of the plates, px when the short side of the frame is 1080 px, 0..64.
    pub radius: f64,
}

impl Default for TextBody {
    fn default() -> Self {
        Self {
            text: "Текст".to_owned(),
            font: FAMILIES[0].to_owned(),
            size: 32.0,
            bold: true,
            italic: false,
            underline: false,
            align: TextAlign::Center,
            color: Rgba([255, 255, 255, 255]),
            background: Rgba([0, 0, 0, 0]),
            radius: 4.0,
        }
    }
}

/// Where the lines stand in the box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    /// Lines start at the left edge.
    Left,
    /// Lines are centred; the default.
    Center,
    /// Lines end at the right edge.
    Right,
}

impl TextAlign {
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

impl From<TextAlign> for Align {
    fn from(align: TextAlign) -> Self {
        match align {
            TextAlign::Left => Align::Left,
            TextAlign::Center => Align::Center,
            TextAlign::Right => Align::Right,
        }
    }
}

/// Between two keys: size and plate radius blend, colours blend by channel, the words, font,
/// style and alignment step at the right key (the same rule `annot key add` applies to stored
/// fields).
impl Blend for TextBody {
    fn blend(&self, to: &Self, e: f64) -> Self {
        Self {
            text: self.text.blend(&to.text, e),
            font: self.font.blend(&to.font, e),
            size: self.size.blend(&to.size, e),
            bold: self.bold.blend(&to.bold, e),
            italic: self.italic.blend(&to.italic, e),
            underline: self.underline.blend(&to.underline, e),
            align: step(&self.align, &to.align, e),
            color: self.color.blend(&to.color, e),
            background: self.background.blend(&to.background, e),
            radius: self.radius.blend(&to.radius, e),
        }
    }
}
