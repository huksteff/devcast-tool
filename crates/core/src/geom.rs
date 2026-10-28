//! Geometry in normalized frame coordinates: (0, 0) is the top-left corner of the output frame,
//! (1, 1) the bottom-right. Values outside 0..1 are allowed (objects partly off-frame).
//!
//! Rotation is applied in pixel space of a concrete frame size, so a rotated rectangle keeps its
//! shape on non-square frames; that is why rotation helpers take a [`PixelSize`].

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

/// A point in normalized frame coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Frame size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

/// Axis-aligned rectangle: top-left corner and size, normalized; size is never negative.
/// Stored as `{x, y, w, h}`; also read from the text `x,y,w,h` (CLI: `--rect 0.1,0,0.8,1`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(into = "RawRect")]
pub struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RectFields")]
struct RawRect {
    /// Left edge, fraction of frame width.
    x: f64,
    /// Top edge, fraction of frame height.
    y: f64,
    /// Width, fraction of frame width (not negative).
    w: f64,
    /// Height, fraction of frame height (not negative).
    h: f64,
}

/// A rectangle with non-finite coordinates or negative size.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error(
    "неверный прямоугольник x={x} y={y} w={w} h={h}: числа должны быть конечными, размеры — не отрицательными"
)]
pub struct InvalidRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Result<Self, InvalidRect> {
        let finite = [x, y, w, h].iter().all(|v| v.is_finite());
        if !finite || w < 0.0 || h < 0.0 {
            return Err(InvalidRect { x, y, w, h });
        }
        Ok(Self { x, y, w, h })
    }

    /// The whole frame.
    pub const FULL: Self = Self {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    };

    pub const fn x(&self) -> f64 {
        self.x
    }
    pub const fn y(&self) -> f64 {
        self.y
    }
    pub const fn w(&self) -> f64 {
        self.w
    }
    pub const fn h(&self) -> f64 {
        self.h
    }

    pub fn center(&self) -> Point {
        Point {
            x: self.x + self.w / 2.0,
            y: self.y + self.h / 2.0,
        }
    }

    /// Edges included on the left/top, excluded on the right/bottom.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }

    /// Pixel rectangle `(x, y, w, h)` in a frame of `size`, as floats (no rounding).
    pub fn to_pixels(&self, size: PixelSize) -> [f64; 4] {
        let (fw, fh) = (f64::from(size.width), f64::from(size.height));
        [self.x * fw, self.y * fh, self.w * fw, self.h * fh]
    }

    /// The four corners after rotating by `degrees` (clockwise on screen) around the center,
    /// computed in pixels of `size`; order: top-left, top-right, bottom-right, bottom-left.
    pub fn rotated_corners(&self, degrees: f64, size: PixelSize) -> [Point; 4] {
        let corners = [
            Point {
                x: self.x,
                y: self.y,
            },
            Point {
                x: self.x + self.w,
                y: self.y,
            },
            Point {
                x: self.x + self.w,
                y: self.y + self.h,
            },
            Point {
                x: self.x,
                y: self.y + self.h,
            },
        ];
        corners.map(|c| rotate_about(c, self.center(), degrees, size))
    }

    /// Hit test for the rectangle rotated by `degrees` around its center.
    pub fn contains_rotated(&self, p: Point, degrees: f64, size: PixelSize) -> bool {
        self.contains(rotate_about(p, self.center(), -degrees, size))
    }
}

/// Text that is not `x,y,w,h`, or a rectangle that [`Rect::new`] rejects.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ParseRectError {
    #[error("«{0}» — не прямоугольник: ожидается x,y,w,h в долях кадра, например 0.1,0,0.8,1")]
    Format(String),
    #[error(transparent)]
    Invalid(#[from] InvalidRect),
}

impl FromStr for Rect {
    type Err = ParseRectError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let numbers: Vec<f64> = text
            .split(',')
            .map(|part| part.trim().parse())
            .collect::<Result<_, _>>()
            .map_err(|_| ParseRectError::Format(text.to_owned()))?;
        let [x, y, w, h] = numbers[..] else {
            return Err(ParseRectError::Format(text.to_owned()));
        };
        Ok(Self::new(x, y, w, h)?)
    }
}

impl<'de> Deserialize<'de> for Rect {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(RectVisitor)
    }
}

struct RectVisitor;

impl<'de> Visitor<'de> for RectVisitor {
    type Value = Rect;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("прямоугольник \"x,y,w,h\" или {x, y, w, h}")
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Rect, E> {
        text.parse().map_err(E::custom)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Rect, A::Error> {
        let raw = RawRect::deserialize(de::value::MapAccessDeserializer::new(map))?;
        Rect::new(raw.x, raw.y, raw.w, raw.h).map_err(de::Error::custom)
    }
}

impl JsonSchema for Rect {
    fn schema_name() -> Cow<'static, str> {
        "Rect".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let fields = generator.subschema_for::<RawRect>();
        json_schema!({
            "anyOf": [
                {
                    "type": "string",
                    "description": "x,y,w,h in frame fractions, e.g. 0.1,0,0.8,1",
                    "pattern": "^[^,]+,[^,]+,[^,]+,[^,]+$"
                },
                fields
            ]
        })
    }
}

impl From<Rect> for RawRect {
    fn from(r: Rect) -> Self {
        Self {
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
        }
    }
}

/// Rotates normalized point `p` around `pivot` by `degrees` (clockwise on screen, y down),
/// with the rotation done in pixel space so angles are true on any aspect ratio.
pub fn rotate_about(p: Point, pivot: Point, degrees: f64, size: PixelSize) -> Point {
    let (fw, fh) = (f64::from(size.width.max(1)), f64::from(size.height.max(1)));
    let (dx, dy) = ((p.x - pivot.x) * fw, (p.y - pivot.y) * fh);
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (rx, ry) = (dx * cos - dy * sin, dx * sin + dy * cos);
    Point {
        x: pivot.x + rx / fw,
        y: pivot.y + ry / fh,
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const HD: PixelSize = PixelSize {
        width: 1920,
        height: 1080,
    };

    fn close(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    #[test]
    fn rejects_negative_size_and_nan() {
        assert!(Rect::new(0.0, 0.0, -0.1, 0.5).is_err());
        assert!(Rect::new(f64::NAN, 0.0, 0.1, 0.1).is_err());
        let error = serde_json::from_str::<Rect>(r#"{"x":0,"y":0,"w":-1,"h":1}"#).unwrap_err();
        assert!(error.to_string().contains("не отрицательными"), "{error}");
    }

    #[test]
    fn reads_text_and_fields_and_writes_fields() {
        let r = Rect::new(0.1, 0.0, 0.8, 1.0).unwrap();
        assert_eq!("0.1,0,0.8,1".parse::<Rect>().unwrap(), r);
        assert_eq!(" 0.1, 0 ,0.8,1 ".parse::<Rect>().unwrap(), r);
        assert_eq!(serde_json::from_str::<Rect>(r#""0.1,0,0.8,1""#).unwrap(), r);
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, r#"{"x":0.1,"y":0.0,"w":0.8,"h":1.0}"#);
        assert_eq!(serde_json::from_str::<Rect>(&json).unwrap(), r);
    }

    #[test]
    fn bad_text_explains_the_form() {
        for text in ["0.1,0,0.8", "0.1,0,0.8,1,2", "a,b,c,d", ""] {
            let error = text.parse::<Rect>().unwrap_err().to_string();
            assert!(error.contains("ожидается x,y,w,h"), "{text}: {error}");
        }
        let error = serde_json::from_str::<Rect>(r#""0,0,-1,1""#).unwrap_err();
        assert!(error.to_string().contains("не отрицательными"), "{error}");
        let error = serde_json::from_str::<Rect>("5").unwrap_err();
        assert!(error.to_string().contains("x,y,w,h"), "{error}");
    }

    #[test]
    fn pixels_follow_frame_size() {
        let r = Rect::new(0.25, 0.5, 0.5, 0.25).unwrap();
        assert_eq!(r.to_pixels(HD), [480.0, 540.0, 960.0, 270.0]);
    }

    #[test]
    fn quarter_turn_keeps_square_square_on_wide_frame() {
        // A 0.1 x 0.1778 rect is 192x192 px on 1920x1080: a square.
        let r = Rect::new(0.45, 0.4111, 0.1, 0.1778).unwrap();
        let c = r.rotated_corners(90.0, HD);
        let px = |p: Point| (p.x * 1920.0, p.y * 1080.0);
        let (ax, ay) = px(c[0]);
        let (bx, by) = px(c[1]);
        let side = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
        assert!((side - 192.0).abs() < 0.1, "side {side}");
    }

    #[test]
    fn rotated_hit_test() {
        let r = Rect::new(0.4, 0.45, 0.2, 0.1).unwrap();
        let tip = Point {
            x: 0.5,
            y: 0.5 + 0.08 * 1920.0 / 1080.0,
        };
        assert!(!r.contains_rotated(tip, 0.0, HD));
        assert!(r.contains_rotated(tip, 90.0, HD));
    }

    proptest! {
        #[test]
        fn rotation_round_trips(x in -1.0f64..2.0, y in -1.0f64..2.0, deg in -720.0f64..720.0) {
            let pivot = Point { x: 0.5, y: 0.5 };
            let p = Point { x, y };
            let back = rotate_about(rotate_about(p, pivot, deg, HD), pivot, -deg, HD);
            prop_assert!(close(back, p));
        }

        #[test]
        fn rotated_corners_are_inside_rotated_rect(deg in -360.0f64..360.0) {
            let r = Rect::new(0.3, 0.3, 0.4, 0.2).unwrap();
            let center = r.center();
            for corner in r.rotated_corners(deg, HD) {
                // Pull each corner slightly towards the center to stay inside the half-open edges.
                let inside = Point { x: corner.x + (center.x - corner.x) * 0.01, y: corner.y + (center.y - corner.y) * 0.01 };
                prop_assert!(r.contains_rotated(inside, deg, HD));
            }
        }
    }
}
