//! `shape set`: which figure a shape annotation draws and how.

use dc_core::time::Output;
use dc_core::{Command, CommandError, Diff, Op, Project, TimeArg, within};
use dc_fx_annotations::{AnnotId, Kind, edit_kind};
use dc_render::{GradientColors, Rgba};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::{
    BoxPoint, CYCLE_MAX, CYCLE_MIN, Figure, GLOW_MAX, RADIUS_MAX, ShapeBody, WIDTH_MAX, WIDTH_MIN,
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShapeSet {
    /// Shape to change.
    pub id: AnnotId,
    /// arrow, rectangle, ellipse, circle, line or highlight; becoming or leaving a highlight
    /// brings its look (fill, outline, opacity) unless given here.
    pub figure: Option<Figure>,
    /// Tail: start of an arrow or a line, `x,y` in fractions of the box, e.g. 0,1.
    pub tail: Option<String>,
    /// Tip: end of an arrow (its head) or a line, `x,y` in fractions of the box, e.g. 1,0.
    pub tip: Option<String>,
    /// Colour of an arrow, a line, an outline, #rrggbbaa.
    pub color: Option<Rgba>,
    /// Thickness, px when the short side of the frame is 1080 px, 1..64.
    pub width: Option<f64>,
    /// Outline of a rectangle, an ellipse, a circle or a highlight on or off.
    pub outline: Option<bool>,
    /// Fill of a rectangle, an ellipse, a circle or a highlight, #rrggbbaa; alpha 00 removes it.
    pub fill: Option<Rgba>,
    /// Corner radius of a rectangle and a highlight, px when the short side is 1080 px, 0..540.
    pub radius: Option<f64>,
    /// Paint with the gradient instead of a solid colour.
    pub gradient: Option<bool>,
    /// 2 to 4 colours of the gradient, comma-separated: #ff4d9dff,#22d3eeff.
    pub colors: Option<String>,
    /// Direction of the gradient, degrees clockwise from left to right in the figure's axes.
    pub angle: Option<f64>,
    /// Seconds per loop of the flowing gradient, 0.25..60; 0 stops it.
    pub cycle: Option<f64>,
    /// Glow, 0..40; 0 removes it.
    pub glow: Option<f64>,
    /// Opacity of the whole figure, 0..1.
    pub opacity: Option<f64>,
    /// Moment on the output video: the fields change in the key there (created with what is
    /// shown). Without it they change in the annotation and in every key.
    pub at: Option<TimeArg<Output>>,
}

impl ShapeSet {
    /// `old` with the given fields replaced, each checked.
    fn change(&self, old: &ShapeBody) -> Result<ShapeBody, CommandError> {
        let mut new = old.clone();
        new.color = self.color.unwrap_or(new.color);
        if let Some(figure) = self.figure {
            new = new.refigured(figure);
        }
        if let Some(tail) = &self.tail {
            new.from = point("начало", tail)?;
        }
        if let Some(tip) = &self.tip {
            new.to = point("конец", tip)?;
        }
        if let Some(width) = self.width {
            new.width = within("толщина", width, WIDTH_MIN, WIDTH_MAX)?;
        }
        if let Some(radius) = self.radius {
            new.radius = within("радиус углов", radius, 0.0, RADIUS_MAX)?;
        }
        if let Some(colors) = &self.colors {
            new.colors = colors
                .parse::<GradientColors>()
                .map_err(|e| CommandError::new(e.to_string()))?;
        }
        if let Some(angle) = self.angle {
            new.angle = within("направление градиента", angle, -360.0, 360.0)?;
        }
        if let Some(cycle) = self.cycle {
            new.cycle = check_cycle(cycle)?;
        }
        if let Some(glow) = self.glow {
            new.glow = within("свечение", glow, 0.0, GLOW_MAX)?;
        }
        if let Some(opacity) = self.opacity {
            new.opacity = within("прозрачность", opacity, 0.0, 1.0)?;
        }
        new.outline = self.outline.unwrap_or(new.outline);
        new.fill = self.fill.unwrap_or(new.fill);
        new.gradient = self.gradient.unwrap_or(new.gradient);
        Ok(new)
    }
}

impl Command for ShapeSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let wrong = "не фигура: shape set меняет только фигуры (devcast ls annots покажет виды)";
        let at = self.at.map(|TimeArg(t)| t);
        edit_kind(
            project,
            self.id,
            Kind::Shape,
            wrong,
            at,
            |old: &ShapeBody| self.change(old),
        )
    }
}

impl Op for ShapeSet {
    const NAME: &'static str = "shape set";
    const ABOUT: &'static str = "Изменить фигуру: вид, концы стрелки, цвета, толщину, заливку, градиент, свечение, прозрачность (с --at — в ключе)";
    const EXAMPLE: &'static str =
        "shape set a2 --figure arrow --tail 0,1 --tip 1,0 --width 8 --color \"#ef4444ff\"";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

/// `x,y` in fractions of the box, each 0..1.
fn point(what: &str, text: &str) -> Result<BoxPoint, CommandError> {
    let numbers: Vec<f64> = text
        .split(',')
        .map(|part| part.trim().parse())
        .collect::<Result<_, _>>()
        .unwrap_or_default();
    match numbers[..] {
        [x, y] if (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) => Ok(BoxPoint { x, y }),
        _ => Err(CommandError::new(format!(
            "{what} фигуры «{text}»: нужно x,y в долях бокса от 0 до 1, например 0,0.5 (бокс задаёт annot set --rect)"
        ))),
    }
}

fn check_cycle(cycle: f64) -> Result<f64, CommandError> {
    if cycle == 0.0 || (CYCLE_MIN..=CYCLE_MAX).contains(&cycle) {
        return Ok(cycle);
    }
    Err(CommandError::new(format!(
        "цикл градиента: нужно от {CYCLE_MIN} до {CYCLE_MAX} секунд или 0 (градиент стоит), а не {cycle}"
    )))
}
