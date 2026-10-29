//! Text layer: lines of text in a box, laid out and drawn by glyphon (decision 005).

use serde::{Deserialize, Serialize};

use super::{PxRect, Rgba};

/// Text in a box (v1 text annotation, `annotationRenderer.ts`): it wraps at the box width minus
/// `padding` on both sides, the block of lines is centred vertically, and the box cuts what does
/// not fit. Lengths are px of the frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub rect: PxRect,
    /// Pieces of the text with their own colour, one after another (a caption's active word);
    /// `\n` starts a new line.
    pub runs: Vec<TextRun>,
    pub font: Font,
    pub underline: bool,
    pub align: Align,
    /// Distance between baselines in font sizes (v1: 1.4).
    pub line_height: f32,
    pub padding: f32,
    /// A rounded box behind each line.
    pub plate: Option<Plate>,
    /// 0..1, for the text and its plates.
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextRun {
    pub text: String,
    pub color: Rgba,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Font {
    /// Family name of a loaded font: `Inter`, `Montserrat`, `Roboto`, `PT Serif`,
    /// `JetBrains Mono` or one from the project's `assets/`.
    pub family: String,
    /// Px of the frame.
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Box behind a line: the ink of the line and its line height, grown by `pad` (x, y).
/// v1: `pad` = (0.2, 0.1) of the font size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plate {
    pub color: Rgba,
    pub radius: f32,
    pub pad: [f32; 2],
}
