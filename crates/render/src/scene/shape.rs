//! Shape layer: SDF figures with a fill, an outline, opacity and rotation (decision 004).

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use super::PxRect;

/// One figure. The outline is centred on the edge and drawn over the fill; `opacity` applies to
/// the figure as a whole. The glow lies under the figure and over everything painted before it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub geometry: Geometry,
    pub fill: Option<Paint>,
    pub stroke: Option<Stroke>,
    /// 0..1.
    pub opacity: f32,
    /// Degrees, clockwise on screen, around the centre of the figure.
    pub rotation: f32,
    pub glow: Option<Glow>,
}

/// Soft light around what is visible of a figure (fill and outline): their coverage in `color`,
/// blurred by a gaussian of `sigma` px of the frame (v1: canvas `shadowBlur` = 2σ).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Glow {
    pub color: Rgba,
    pub sigma: f32,
}

/// Outline of a figure, px of the frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Geometry {
    /// Rectangle with corners rounded by `radius` (at most half the shorter side).
    Rect { rect: PxRect, radius: f32 },
    /// Ellipse inscribed in `rect`; a circle when it is square.
    Ellipse { rect: PxRect },
    /// Segment of `width` with round caps; the fill paints it.
    Line {
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
    },
    /// A line with two whiskers of length `head` at ±45° at `to` (the straight arrow of v1).
    Arrow {
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
        head: f32,
    },
}

/// How a fill is coloured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Paint {
    Solid(Rgba),
    Gradient(Gradient),
}

/// Linear gradient of evenly spaced colours across the figure's bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    pub colors: GradientColors,
    /// Direction in degrees, clockwise from left-to-right, in the figure's own axes.
    pub angle: f32,
    /// `None`: the colours run once from one edge to the other. `Some(phase)`: they repeat in a
    /// seamless loop (the last flows into the first) shifted back by `phase` of the bounds, so a
    /// growing phase moves the pattern against the direction (v1 animated gradient).
    pub cycle: Option<f32>,
}

/// Colours of a gradient, from 2 to 4 (SCOPE).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Rgba>", into = "Vec<Rgba>")]
pub struct GradientColors(Vec<Rgba>);

impl GradientColors {
    pub fn colors(&self) -> &[Rgba] {
        &self.0
    }
}

/// A gradient with fewer than 2 or more than 4 colours.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("в градиенте {0} цветов, а нужно от 2 до 4")]
pub struct GradientCount(usize);

impl TryFrom<Vec<Rgba>> for GradientColors {
    type Error = GradientCount;

    fn try_from(colors: Vec<Rgba>) -> Result<Self, Self::Error> {
        if (2..=4).contains(&colors.len()) {
            Ok(Self(colors))
        } else {
            Err(GradientCount(colors.len()))
        }
    }
}

impl From<GradientColors> for Vec<Rgba> {
    fn from(colors: GradientColors) -> Self {
        colors.0
    }
}

/// Outline centred on the edge of the figure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    /// px of the frame.
    pub width: f32,
    pub color: Rgba,
}

/// sRGB colour with straight alpha, written `#rrggbbaa`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Rgba(pub [u8; 4]);

/// Text that is not `#rrggbbaa`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("«{0}» — не цвет: нужен #rrggbbaa, например #14b8c4ff")]
pub struct ParseRgbaError(String);

impl FromStr for Rgba {
    type Err = ParseRgbaError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = || ParseRgbaError(text.to_owned());
        let hex = text
            .strip_prefix('#')
            .filter(|h| h.len() == 8 && h.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(error)?;
        let mut rgba = [0; 4];
        for (i, channel) in rgba.iter_mut().enumerate() {
            let pair = hex.get(i * 2..i * 2 + 2).ok_or_else(error)?;
            *channel = u8::from_str_radix(pair, 16).map_err(|_| error())?;
        }
        Ok(Self(rgba))
    }
}

impl TryFrom<String> for Rgba {
    type Error = ParseRgbaError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<Rgba> for String {
    fn from(color: Rgba) -> Self {
        color.to_string()
    }
}

/// Effect states keep colours too (click effects, callout), so the type has a schema.
impl JsonSchema for Rgba {
    fn schema_name() -> Cow<'static, str> {
        "Rgba".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "sRGB colour with alpha, #rrggbbaa, e.g. #2563ebff",
            "pattern": "^#[0-9a-fA-F]{8}$"
        })
    }
}

impl Rgba {
    /// Linear rgb and alpha (straight).
    pub(crate) fn linear(self) -> [f32; 4] {
        let [r, g, b, a] = self.0.map(|c| f32::from(c) / 255.0);
        let decode = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        [decode(r), decode(g), decode(b), a]
    }
}

impl fmt::Display for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [r, g, b, a] = self.0;
        write!(f, "#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_reads_and_writes_hex() {
        let color: Rgba = "#14B8c4ff".parse().unwrap();
        assert_eq!(color, Rgba([0x14, 0xb8, 0xc4, 0xff]));
        assert_eq!(color.to_string(), "#14b8c4ff");
        for bad in [
            "14b8c4ff",
            "#14b8c4",
            "#14b8c4fff",
            "#14b8c4gg",
            "#+fb8c4ff",
            "#ффb8c4ff",
        ] {
            assert!(bad.parse::<Rgba>().is_err(), "{bad}");
        }
    }

    #[test]
    fn srgb_turns_linear() {
        let [r, g, b, a] = Rgba([255, 188, 0, 128]).linear();
        assert!((r - 1.0).abs() < 1e-6 && b == 0.0);
        assert!((g - 0.5).abs() < 0.003, "{g}");
        assert!((a - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn gradient_takes_two_to_four_colors() {
        let red = Rgba([255, 0, 0, 255]);
        assert!(GradientColors::try_from(vec![red]).is_err());
        assert!(GradientColors::try_from(vec![red; 2]).is_ok());
        assert!(GradientColors::try_from(vec![red; 4]).is_ok());
        assert!(GradientColors::try_from(vec![red; 5]).is_err());
        let error = serde_json::from_str::<GradientColors>(r##"["#ff0000ff"]"##).unwrap_err();
        assert!(error.to_string().contains("от 2 до 4"), "{error}");
    }

    #[test]
    fn shape_round_trips_through_json() {
        let shape = Shape {
            geometry: Geometry::Arrow {
                from: [100.0, 200.0],
                to: [400.0, 200.0],
                width: 8.0,
                head: 42.0,
            },
            fill: Some(Paint::Gradient(Gradient {
                colors: vec![Rgba([0x14, 0xb8, 0xc4, 255]), Rgba([0xa8, 0x55, 0xf7, 255])]
                    .try_into()
                    .unwrap(),
                angle: 0.0,
                cycle: Some(0.25),
            })),
            stroke: None,
            opacity: 1.0,
            rotation: 0.0,
            glow: Some(Glow {
                color: Rgba([0xa8, 0x55, 0xf7, 255]),
                sigma: 10.5,
            }),
        };
        let json = serde_json::to_string(&shape).unwrap();
        assert!(
            json.contains(r##""colors":["#14b8c4ff","#a855f7ff"]"##),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<Shape>(&json).unwrap(), shape);
    }
}
