//! The fields of a shape annotation, kept in `Annotation::body` (fx/annotations).

use dc_fx_annotations::{Blend, step};
use dc_render::{BLUE, GradientColors, Rgba};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Thickness of an arrow, a line and an outline, px when the short side of the frame is 1080 px.
pub const WIDTH_MIN: f64 = 1.0;
pub const WIDTH_MAX: f64 = 64.0;
/// Corner radius of a rectangle and a highlight, px when the short side of the frame is 1080 px.
pub const RADIUS_MAX: f64 = 540.0;
/// Glow strength (v1 units: σ = 0.75·glow ref-px).
pub const GLOW_MAX: f64 = 40.0;
/// Seconds for one loop of an animated gradient; 0 is a still gradient.
pub const CYCLE_MIN: f64 = 0.25;
pub const CYCLE_MAX: f64 = 60.0;
/// Opacity of a highlight when a figure becomes one (v1).
pub const HIGHLIGHT_OPACITY: f64 = 0.28;

/// What a shape annotation draws; a missing field takes its default (v1: a blue arrow pointing
/// right across the box).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ShapeBody {
    /// Which figure: arrow, rectangle, ellipse, circle, line or highlight.
    pub figure: Figure,
    /// Start of an arrow or a line in its box; the box moves and turns the figure.
    pub from: BoxPoint,
    /// End of an arrow (its tip) or a line in its box.
    pub to: BoxPoint,
    /// Colour of an arrow, a line and of the outline of the other figures.
    pub color: Rgba,
    /// Thickness of an arrow, a line and an outline, px when the short side of the frame is
    /// 1080 px, 1..64; it does not grow with the box (decision 000).
    pub width: f64,
    /// Outline of a rectangle, an ellipse, a circle or a highlight (an arrow and a line are
    /// always drawn).
    pub outline: bool,
    /// Fill of a rectangle, an ellipse, a circle or a highlight; alpha 00 means no fill.
    pub fill: Rgba,
    /// Corner radius of a rectangle and a highlight, px when the short side of the frame is
    /// 1080 px, 0..540.
    pub radius: f64,
    /// Paint with a gradient of `colors` instead of a solid colour: the fill of a closed figure,
    /// the whole arrow or line.
    pub gradient: bool,
    /// 2 to 4 colours of the gradient.
    pub colors: GradientColors,
    /// Direction of the gradient, degrees clockwise from left to right in the figure's own axes
    /// (an arrow's: from its start to its tip).
    pub angle: f64,
    /// Seconds for the colours to flow through one loop, by the output video (0.25..60); 0 is a
    /// still gradient running once from edge to edge.
    pub cycle: f64,
    /// Glow around the figure, 0..40 (σ = 0.75·glow px at 1080), in the gradient's first colour
    /// or the figure's own; 0 means none. Works without a gradient (decision 000).
    pub glow: f64,
    /// Opacity of the whole figure, 0..1.
    pub opacity: f64,
}

/// v1 defaults: blue `#2563eb`, gradient colours `#ff4d9d`, `#22d3ee` flowing in 2.4 s.
impl Default for ShapeBody {
    fn default() -> Self {
        Self {
            figure: Figure::Arrow,
            from: BoxPoint { x: 0.0, y: 0.5 },
            to: BoxPoint { x: 1.0, y: 0.5 },
            color: BLUE,
            width: 6.0,
            outline: true,
            fill: Rgba([BLUE.0[0], BLUE.0[1], BLUE.0[2], 0]),
            radius: 8.0,
            gradient: false,
            colors: GradientColors::two(
                Rgba([0xff, 0x4d, 0x9d, 255]),
                Rgba([0x22, 0xd3, 0xee, 255]),
            ),
            angle: 0.0,
            cycle: 2.4,
            glow: 0.0,
            opacity: 1.0,
        }
    }
}

impl ShapeBody {
    /// The look a figure comes with when it becomes or stops being a highlight (v1 creates a
    /// highlight filled in its colour, 28 % opaque, without an outline): fill, outline and
    /// opacity; the command sets what it gives after this.
    pub fn refigured(&self, figure: Figure) -> Self {
        let mut new = self.clone();
        new.figure = figure;
        if (self.figure == Figure::Highlight) == (figure == Figure::Highlight) {
            return new;
        }
        let [r, g, b, _] = self.color.0;
        let highlight = figure == Figure::Highlight;
        new.fill = Rgba([r, g, b, if highlight { 255 } else { 0 }]);
        new.outline = !highlight;
        new.opacity = if highlight { HIGHLIGHT_OPACITY } else { 1.0 };
        new
    }
}

/// A point of the box (stored as an object, so a key snapshot blends it like the frame does).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BoxPoint {
    /// Fraction of the box width from its left side, 0..1.
    pub x: f64,
    /// Fraction of the box height from its top, 0..1.
    pub y: f64,
}

impl Blend for BoxPoint {
    fn blend(&self, to: &Self, e: f64) -> Self {
        Self {
            x: self.x.blend(&to.x, e),
            y: self.y.blend(&to.y, e),
        }
    }
}

/// The figure a shape annotation draws in its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Figure {
    /// From `from` to its tip at `to`, two whiskers at ±45°.
    Arrow,
    /// The box, corners rounded by `radius`.
    Rectangle,
    /// Inscribed in the box.
    Ellipse,
    /// The largest circle in the middle of the box.
    Circle,
    /// From `from` to `to`, round ends.
    Line,
    /// A rectangle that marks an area: filled and see-through.
    Highlight,
}

impl Figure {
    pub fn name(self) -> &'static str {
        match self {
            Self::Arrow => "arrow",
            Self::Rectangle => "rectangle",
            Self::Ellipse => "ellipse",
            Self::Circle => "circle",
            Self::Line => "line",
            Self::Highlight => "highlight",
        }
    }

    /// An arrow or a line: drawn along `from`→`to`, no fill or outline of its own.
    pub fn is_open(self) -> bool {
        matches!(self, Self::Arrow | Self::Line)
    }
}

/// Between two keys: points, thickness, radius, angle, loop, glow and opacity blend, colours
/// blend by channel (gradient colours too when both keys have as many), the figure and the
/// switches step at the right key.
impl Blend for ShapeBody {
    fn blend(&self, to: &Self, e: f64) -> Self {
        Self {
            figure: step(&self.figure, &to.figure, e),
            from: self.from.blend(&to.from, e),
            to: self.to.blend(&to.to, e),
            color: self.color.blend(&to.color, e),
            width: self.width.blend(&to.width, e),
            outline: self.outline.blend(&to.outline, e),
            fill: self.fill.blend(&to.fill, e),
            radius: self.radius.blend(&to.radius, e),
            gradient: self.gradient.blend(&to.gradient, e),
            colors: blend_colors(&self.colors, &to.colors, e),
            angle: self.angle.blend(&to.angle, e),
            cycle: self.cycle.blend(&to.cycle, e),
            glow: self.glow.blend(&to.glow, e),
            opacity: self.opacity.blend(&to.opacity, e),
        }
    }
}

fn blend_colors(from: &GradientColors, to: &GradientColors, e: f64) -> GradientColors {
    let (a, b) = (from.colors(), to.colors());
    if a.len() != b.len() {
        return step(from, to, e);
    }
    let mixed: Vec<Rgba> = a.iter().zip(b).map(|(a, b)| a.blend(b, e)).collect();
    GradientColors::try_from(mixed).unwrap_or_else(|_| step(from, to, e))
}
