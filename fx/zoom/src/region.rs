//! `zoom add`, `zoom set`, `zoom rm`: zoom regions.

use dc_core::time::Output;
use dc_core::{Command, CommandError, Diff, Op, Point, Project, TimeArg, TimeDelta, index_of};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::{
    DEFAULT_SCALE, Mode, SCALE_MAX, SCALE_MIN, Zoom, ZoomEffect, ZoomId, ZoomState,
};
use crate::span::{span, timeline};

fn one_second() -> f64 {
    1.0
}

fn default_scale() -> f64 {
    DEFAULT_SCALE
}

fn center() -> f64 {
    0.5
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ZoomAdd {
    /// Start on the output video. `clicks` (CLI only) — the zooms of `devcast zoom suggest`
    /// on clusters of clicks, one undo step; then only `scale` and `mode` apply.
    pub at: TimeArg<Output>,
    /// Length, seconds of the output video.
    #[serde(default = "one_second")]
    pub duration: f64,
    /// Magnification, 1..5.
    #[serde(default = "default_scale")]
    pub scale: f64,
    /// follow (by default) the cursor or stay fixed on the focus.
    #[serde(default)]
    pub mode: Mode,
    /// Focus, fraction of the frame width (0..1).
    #[serde(default = "center")]
    pub x: f64,
    /// Focus, fraction of the frame height (0..1).
    #[serde(default = "center")]
    pub y: f64,
}

impl Command for ZoomAdd {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let timeline = timeline(project)?;
        let start = timeline.start(self.at.0)?;
        let end = timeline.end(self.at.0 + TimeDelta::from_secs_f64(self.duration))?;
        let (media, range) = span(&timeline, start, end)?;
        check_scale(self.scale)?;
        let focus = Point {
            x: check_focus("x", self.x)?,
            y: check_focus("y", self.y)?,
        };
        let mut state = project.state::<ZoomEffect>()?;
        let zoom = Zoom {
            id: state.ids.allocate(),
            media,
            range,
            scale: self.scale,
            focus,
            mode: self.mode,
        };
        check_overlap(&state, &zoom)?;
        let id = zoom.id;
        state.zooms.push(zoom);
        project.set_state::<ZoomEffect>(&state)?;
        Ok(Diff::added(id))
    }
}

impl Op for ZoomAdd {
    const NAME: &'static str = "zoom add";
    const ABOUT: &'static str = "Добавить зум (время — по готовому видео)";
    const EXAMPLE: &'static str = "zoom add --at 12.5 --duration 2 --scale 1.8";
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ZoomSet {
    /// Zoom to change.
    pub id: ZoomId,
    /// New start on the output video.
    pub from: Option<TimeArg<Output>>,
    /// New end on the output video.
    pub to: Option<TimeArg<Output>>,
    /// Magnification, 1..5.
    pub scale: Option<f64>,
    /// follow the cursor or stay fixed on the focus.
    pub mode: Option<Mode>,
    /// Focus, fraction of the frame width (0..1).
    pub x: Option<f64>,
    /// Focus, fraction of the frame height (0..1).
    pub y: Option<f64>,
}

impl Command for ZoomSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<ZoomEffect>()?;
        let index = position(&state, self.id)?;
        let mut zoom = state.zooms[index].clone();
        if self.from.is_some() || self.to.is_some() {
            let timeline = timeline(project)?;
            let start = match self.from {
                Some(TimeArg(t)) => timeline.start(t)?,
                None => (zoom.media.clone(), zoom.range.start()),
            };
            let end = match self.to {
                Some(TimeArg(t)) => timeline.end(t)?,
                None => (zoom.media.clone(), zoom.range.end()),
            };
            (zoom.media, zoom.range) = span(&timeline, start, end)?;
        }
        if let Some(scale) = self.scale {
            zoom.scale = check_scale(scale)?;
        }
        if let Some(x) = self.x {
            zoom.focus.x = check_focus("x", x)?;
        }
        if let Some(y) = self.y {
            zoom.focus.y = check_focus("y", y)?;
        }
        zoom.mode = self.mode.unwrap_or(zoom.mode);
        if zoom == state.zooms[index] {
            return Ok(Diff::default());
        }
        check_overlap(&state, &zoom)?;
        state.zooms[index] = zoom;
        project.set_state::<ZoomEffect>(&state)?;
        Ok(Diff::changed(self.id))
    }
}

impl Op for ZoomSet {
    const NAME: &'static str = "zoom set";
    const ABOUT: &'static str = "Изменить зум: границы, масштаб, режим, точку фокуса";
    const EXAMPLE: &'static str = "zoom set z3 --scale 2 --mode fixed --x 0.3 --y 0.4";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ZoomRm {
    /// Zoom to remove.
    pub id: ZoomId,
}

impl Command for ZoomRm {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<ZoomEffect>()?;
        let index = position(&state, self.id)?;
        state.zooms.remove(index);
        project.set_state::<ZoomEffect>(&state)?;
        Ok(Diff::removed(self.id))
    }
}

impl Op for ZoomRm {
    const NAME: &'static str = "zoom rm";
    const ABOUT: &'static str = "Удалить зум";
    const EXAMPLE: &'static str = "zoom rm z3";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

fn position(state: &ZoomState, id: ZoomId) -> Result<usize, CommandError> {
    index_of(&state.zooms, id, |z| z.id)
}

fn check_scale(scale: f64) -> Result<f64, CommandError> {
    if (SCALE_MIN..=SCALE_MAX).contains(&scale) {
        return Ok(scale);
    }
    Err(CommandError::new(format!(
        "масштаб должен быть от {SCALE_MIN} до {SCALE_MAX}, а не {scale}"
    )))
}

fn check_focus(axis: &str, value: f64) -> Result<f64, CommandError> {
    if (0.0..=1.0).contains(&value) {
        return Ok(value);
    }
    Err(CommandError::new(format!(
        "{axis} фокуса должен быть в 0..1 (доля кадра), а не {value}"
    )))
}

fn check_overlap(state: &ZoomState, zoom: &Zoom) -> Result<(), CommandError> {
    let other = state.zooms.iter().find(|z| {
        z.id != zoom.id && z.media == zoom.media && z.range.intersect(&zoom.range).is_some()
    });
    match other {
        Some(other) => Err(CommandError::new(format!(
            "зум перекрывается с {}: сдвинь или укороти его; список: devcast ls zooms",
            other.id
        ))),
        None => Ok(()),
    }
}
