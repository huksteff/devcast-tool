//! Text layer: lines of text in a box, laid out and drawn by glyphon (decision 005).

use serde::{Deserialize, Serialize};

use super::{Glow, Paint, PxRect, Rgba};

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
    /// One line put next to a point instead of centred in `rect` (the cursor callout).
    pub beside: Option<Beside>,
    pub shadow: Option<Shadow>,
    /// Degrees clockwise around the centre of `rect` (a text annotation): the box, its cut, the
    /// plates and the shadow turn with the letters.
    pub rotation: f32,
}

/// Soft shadow of the letters (captions): their shape in `color`, moved by `offset` and blurred
/// by a gaussian of `sigma`, under the glyphs and over the plates. Px of the frame; the camera
/// scales them with the text. v1: canvas `shadowBlur` = 2σ.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub color: Rgba,
    pub sigma: f32,
    pub offset: [f32; 2],
}

/// Places one line (no wrapping) by a point. Its box — the ink grown by the plate's `pad`, or by
/// `padding` without a plate — gets its top-left corner at `point + offset`. On an axis where the
/// box would come closer than `margin` to the edge of `rect`, it goes to the other side of the
/// point (its far corner at `point − offset`); if it fits neither way, it is pushed inside `rect`.
/// `rect` also cuts the text. Px of the frame, like `rect`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Beside {
    pub point: [f32; 2],
    pub offset: [f32; 2],
    pub margin: f32,
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

/// Families of the built-in fonts (decision 000), the same for captions and annotations.
pub const FAMILIES: [&str; 5] = [
    "Inter",
    "Montserrat",
    "Roboto",
    "PT Serif",
    "JetBrains Mono",
];

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
    /// A gradient runs across each line's box.
    pub paint: Paint,
    pub radius: f32,
    pub pad: [f32; 2],
    pub glow: Option<Glow>,
}
