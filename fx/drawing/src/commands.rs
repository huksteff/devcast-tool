//! `drawing add|set|rm|clear`: the strokes of a drawing annotation, given as points of its box
//! (an agent draws with numbers, the editor with the mouse, 7.24).

use dc_core::time::Output;
use dc_core::{Command, CommandError, Diff, Op, Project, TimeArg, within};
use dc_fx_annotations::{AnnotId, Kind, edit_kind};
use dc_render::Rgba;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::{BLUE, DrawingBody, Stroke, StrokeTool, WIDTH_MAX, WIDTH_MIN};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DrawingAdd {
    pub id: AnnotId,
    /// Points of the stroke in fractions of the box, `x1,y1,x2,y2,…` each 0..1: one point is a
    /// dot, two points a straight line.
    pub points: String,
    /// pencil (by default) or eraser: an eraser clears what earlier strokes of this drawing
    /// painted under it.
    pub tool: Option<StrokeTool>,
    /// Colour of a pencil stroke, #rrggbbaa; #2563ebff by default.
    pub color: Option<Rgba>,
    /// Thickness, px when the short side of the frame is 1080 px, 1..64; 4 for a pencil and 12
    /// for an eraser by default.
    pub width: Option<f64>,
    /// Moment on the output video: the stroke is added in the key there (created with what is
    /// shown). Without it the stroke is added to the annotation and to every key.
    pub at: Option<TimeArg<Output>>,
}

impl Command for DrawingAdd {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let tool = self.tool.unwrap_or(StrokeTool::Pencil);
        let stroke = Stroke {
            tool,
            color: paint(tool, self.color)?.unwrap_or(BLUE),
            width: self.width.map_or(Ok(tool.width()), width)?,
            points: points(&self.points)?,
        };
        edit(project, self.id, self.at, |old| {
            let mut new = old.clone();
            new.strokes.push(stroke.clone());
            Ok(new)
        })
    }
}

impl Op for DrawingAdd {
    const NAME: &'static str = "drawing add";
    const ABOUT: &'static str = "Добавить штрих в рисунок: карандаш или ластик по точкам бокса, две точки — прямая (с --at — в ключе)";
    const EXAMPLE: &'static str =
        "drawing add a4 --points 0.1,0.8,0.5,0.2,0.9,0.8 --color \"#ef4444ff\" --width 6";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DrawingSet {
    pub id: AnnotId,
    /// Number of the stroke to change, from 1 in drawing order; without it colour and thickness
    /// change every pencil stroke.
    pub stroke: Option<usize>,
    /// Colour of pencil strokes, #rrggbbaa.
    pub color: Option<Rgba>,
    /// Thickness, px when the short side of the frame is 1080 px, 1..64.
    pub width: Option<f64>,
    /// Opacity of the whole drawing, 0..1.
    pub opacity: Option<f64>,
    /// Moment on the output video: the fields change in the key there (created with what is
    /// shown). Without it they change in the annotation and in every key.
    pub at: Option<TimeArg<Output>>,
}

impl DrawingSet {
    fn change(&self, old: &DrawingBody) -> Result<DrawingBody, CommandError> {
        let mut new = old.clone();
        let width = self.width.map(width).transpose()?;
        if let Some(opacity) = self.opacity {
            new.opacity = within("прозрачность рисунка", opacity, 0.0, 1.0)?;
        }
        let strokes: Vec<&mut Stroke> = match self.stroke {
            Some(n) => vec![&mut new.strokes[index(old, n)?]],
            None => new
                .strokes
                .iter_mut()
                .filter(|s| s.tool == StrokeTool::Pencil)
                .collect(),
        };
        for stroke in strokes {
            if let Some(color) = paint(stroke.tool, self.color)? {
                stroke.color = color;
            }
            stroke.width = width.unwrap_or(stroke.width);
        }
        Ok(new)
    }
}

impl Command for DrawingSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        edit(project, self.id, self.at, |old| self.change(old))
    }
}

impl Op for DrawingSet {
    const NAME: &'static str = "drawing set";
    const ABOUT: &'static str = "Изменить рисунок: цвет и толщину штриха или всех штрихов карандаша, прозрачность (с --at — в ключе)";
    const EXAMPLE: &'static str = "drawing set a4 --stroke 2 --color \"#22c55eff\" --width 8";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DrawingRm {
    pub id: AnnotId,
    /// Number of the stroke, from 1 in drawing order.
    pub stroke: usize,
    /// Moment on the output video: the stroke goes from the key there. Without it it goes from
    /// the annotation and from every key.
    pub at: Option<TimeArg<Output>>,
}

impl Command for DrawingRm {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        edit(project, self.id, self.at, |old| {
            let mut new = old.clone();
            new.strokes.remove(index(old, self.stroke)?);
            Ok(new)
        })
    }
}

impl Op for DrawingRm {
    const NAME: &'static str = "drawing rm";
    const ABOUT: &'static str =
        "Убрать штрих рисунка по номеру, остальные сдвигаются (с --at — в ключе)";
    const EXAMPLE: &'static str = "drawing rm a4 2";
    const POSITIONAL: &'static [&'static str] = &["id", "stroke"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DrawingClear {
    pub id: AnnotId,
    /// Moment on the output video: the strokes go from the key there. Without it they go from
    /// the annotation and from every key.
    pub at: Option<TimeArg<Output>>,
}

impl Command for DrawingClear {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        edit(project, self.id, self.at, |old| {
            let mut new = old.clone();
            new.strokes.clear();
            Ok(new)
        })
    }
}

impl Op for DrawingClear {
    const NAME: &'static str = "drawing clear";
    const ABOUT: &'static str = "Стереть все штрихи рисунка (с --at — в ключе)";
    const EXAMPLE: &'static str = "drawing clear a4";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

/// Changes the fields of drawing `id`: in the key at output `at`, else everywhere.
fn edit(
    project: &mut Project,
    id: AnnotId,
    at: Option<TimeArg<Output>>,
    change: impl Fn(&DrawingBody) -> Result<DrawingBody, CommandError>,
) -> Result<Diff, CommandError> {
    let wrong = "не рисунок: команды drawing меняют только рисунки (devcast ls annots покажет виды, рисунок создаёт annot add drawing)";
    edit_kind(
        project,
        id,
        Kind::Drawing,
        wrong,
        at.map(|TimeArg(t)| t),
        change,
    )
}

/// `x1,y1,x2,y2,…` in fractions of the box, each 0..1, at least one point.
fn points(text: &str) -> Result<Vec<[f64; 2]>, CommandError> {
    let numbers: Option<Vec<f64>> = text
        .split(',')
        .map(|part| part.trim().parse().ok())
        .collect();
    match numbers {
        Some(n) if n.len() % 2 == 0 && n.iter().all(|v| (0.0..=1.0).contains(v)) => {
            Ok(n.as_chunks::<2>().0.to_vec())
        }
        _ => Err(CommandError::new(format!(
            "точки штриха «{text}»: нужны пары x,y через запятую в долях бокса от 0 до 1, например 0.1,0.8,0.5,0.2 — прямая из двух точек (бокс задаёт annot set --rect)"
        ))),
    }
}

fn width(value: f64) -> Result<f64, CommandError> {
    within("толщина штриха (px при 1080p)", value, WIDTH_MIN, WIDTH_MAX)
}

/// The colour given for a stroke of `tool`: an eraser has none.
fn paint(tool: StrokeTool, color: Option<Rgba>) -> Result<Option<Rgba>, CommandError> {
    match (tool, color) {
        (StrokeTool::Eraser, Some(_)) => Err(CommandError::new(
            "у ластика нет цвета: убери --color или выбери штрих карандаша (--stroke)",
        )),
        _ => Ok(color),
    }
}

/// Index of stroke number `n` (from 1) of `body`.
fn index(body: &DrawingBody, n: usize) -> Result<usize, CommandError> {
    let count = body.strokes.len();
    if (1..=count).contains(&n) {
        return Ok(n - 1);
    }
    Err(CommandError::new(match count {
        0 => format!("штриха {n} нет: рисунок пуст (штрих добавляет drawing add)"),
        _ => format!(
            "штриха {n} нет: номера штрихов от 1 до {count} (devcast ls annots --json покажет их)"
        ),
    }))
}
