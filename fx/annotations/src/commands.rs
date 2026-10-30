//! `annot add`, `annot set`, `annot rm`: annotations, their time, box and rotation.

use dc_core::time::Output;
use dc_core::{Command, CommandError, Diff, Op, Project, Rect, TimeArg, TimeDelta, index_of};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Map;

use crate::keyed::key_offset;
use crate::model::{
    AnnotId, Annotation, Annotations, AnnotationsState, Kind, POSITION_LIMIT, ROTATION_LIMIT,
    SIZE_MAX, SIZE_MIN, TRACKS,
};
use crate::motion::Stage;
use crate::span::Timeline;

fn one_second() -> f64 {
    1.0
}

/// A box of 0.3×0.2 of the frame around its centre.
fn centered() -> Result<Rect, CommandError> {
    Rect::new(0.35, 0.4, 0.3, 0.2).map_err(|e| CommandError::new(e.to_string()))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnotAdd {
    pub kind: Kind,
    /// Start on the output video.
    pub at: TimeArg<Output>,
    /// Length, seconds of the output video, at least 0.1.
    #[serde(default = "one_second")]
    pub duration: f64,
    /// Track, 0..31; a higher track lies above.
    #[serde(default)]
    pub track: u32,
    /// Box `x,y,w,h` in fractions of the frame (before the zoom); by default 0.3×0.2 in the
    /// centre.
    pub rect: Option<Rect>,
    /// Degrees clockwise around the box centre, -360..360.
    #[serde(default)]
    pub rotation: f64,
}

impl Command for AnnotAdd {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let timeline = Timeline::new(project)?;
        let length = TimeDelta::from_secs_f64(self.duration);
        if !self.duration.is_finite() || length <= TimeDelta::ZERO {
            return Err(CommandError::new(format!(
                "длительность аннотации должна быть больше нуля (секунды), а не {}",
                self.duration
            )));
        }
        let start = timeline.start(self.at.0)?;
        let end = timeline.end(self.at.0 + length)?;
        let (media, range) = timeline.span(start, end)?;
        let mut state = project.state::<Annotations>()?;
        let annotation = Annotation {
            id: state.ids.allocate(),
            kind: self.kind,
            media,
            range,
            track: check_track(self.track)?,
            rect: check_rect(match self.rect {
                Some(rect) => rect,
                None => centered()?,
            })?,
            rotation: check_rotation(self.rotation)?,
            enter: Stage::default(),
            exit: Stage::default(),
            body: Map::new(),
            keys: Vec::new(),
        };
        let id = annotation.id;
        state.annotations.push(annotation);
        project.set_state::<Annotations>(&state)?;
        Ok(Diff::added(id))
    }
}

impl Op for AnnotAdd {
    const NAME: &'static str = "annot add";
    const ABOUT: &'static str =
        "Добавить аннотацию: text, shape, image, drawing или blur (время — по готовому видео)";
    const EXAMPLE: &'static str =
        "annot add text --at 3 --duration 2 --rect 0.1,0.1,0.3,0.1 --rotation -5";
    const POSITIONAL: &'static [&'static str] = &["kind"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnotSet {
    pub id: AnnotId,
    /// New start on the output video.
    pub from: Option<TimeArg<Output>>,
    /// New end on the output video.
    pub to: Option<TimeArg<Output>>,
    /// New box `x,y,w,h` in fractions of the frame; not together with `x`, `y`, `w`, `h`.
    pub rect: Option<Rect>,
    /// Left edge, fraction of the frame width.
    pub x: Option<f64>,
    /// Top edge, fraction of the frame height.
    pub y: Option<f64>,
    /// Width, fraction of the frame width, 0.01..10.
    pub w: Option<f64>,
    /// Height, fraction of the frame height, 0.01..10.
    pub h: Option<f64>,
    /// Degrees clockwise around the box centre, -360..360.
    pub rotation: Option<f64>,
    /// Moment on the output video: the box and rotation change in the key there (created with
    /// what is shown). Without it they change in the annotation and in every key.
    pub at: Option<TimeArg<Output>>,
}

impl AnnotSet {
    fn keyed(&self) -> bool {
        self.rect.is_some()
            || [self.x, self.y, self.w, self.h, self.rotation]
                .iter()
                .any(Option::is_some)
    }

    /// The box and rotation of the annotation, or of its key at `self.at`.
    fn change_frame(&self, project: &Project, a: &mut Annotation) -> Result<(), CommandError> {
        let rotation = self.rotation.map(check_rotation).transpose()?;
        let mut frames = match self.at {
            Some(TimeArg(t)) if self.keyed() => {
                let offset = key_offset(project, a, t)?;
                let shown = a.frame_at(offset);
                let new = (self.rect(shown.0)?, rotation.unwrap_or(shown.1));
                if new != shown {
                    let key = a.edit_key(offset);
                    (key.rect, key.rotation) = new;
                }
                Vec::new()
            }
            Some(_) => Vec::new(),
            None => std::iter::once((&mut a.rect, &mut a.rotation))
                .chain(a.keys.iter_mut().map(|k| (&mut k.rect, &mut k.rotation)))
                .collect(),
        };
        for (rect, turn) in &mut frames {
            **rect = self.rect(**rect)?;
            **turn = rotation.unwrap_or(**turn);
        }
        Ok(())
    }

    fn rect(&self, old: Rect) -> Result<Rect, CommandError> {
        let parts = [self.x, self.y, self.w, self.h];
        let Some(rect) = self.rect else {
            let [x, y, w, h] = parts;
            let rect = Rect::new(
                x.unwrap_or(old.x()),
                y.unwrap_or(old.y()),
                w.unwrap_or(old.w()),
                h.unwrap_or(old.h()),
            )
            .map_err(|e| CommandError::new(e.to_string()))?;
            return check_rect(rect);
        };
        if parts.iter().any(Option::is_some) {
            return Err(CommandError::new(
                "укажи либо --rect, либо --x/--y/--w/--h, но не вместе",
            ));
        }
        check_rect(rect)
    }
}

impl Command for AnnotSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Annotations>()?;
        let index = position(&state, self.id)?;
        let mut annotation = state.annotations[index].clone();
        if self.from.is_some() || self.to.is_some() {
            if self.at.is_some() {
                return Err(CommandError::new(
                    "время аннотации ключами не меняется: --from и --to укажи без --at",
                ));
            }
            let timeline = Timeline::new(project)?;
            let start_edge = match self.from {
                Some(TimeArg(t)) => timeline.start(t)?,
                None => (annotation.media.clone(), annotation.range.start()),
            };
            let end = match self.to {
                Some(TimeArg(t)) => timeline.end(t)?,
                None => (annotation.media.clone(), annotation.range.end()),
            };
            let (media, start) = (annotation.media.clone(), annotation.range.start());
            (annotation.media, annotation.range) = timeline.span(start_edge, end)?;
            if annotation.media == media {
                // Keys stay on their frames of the recording when the head is trimmed.
                let shift = annotation.range.start().since(start).micros();
                for key in &mut annotation.keys {
                    key.at = TimeDelta::from_micros(key.at.micros() - shift);
                }
            }
        }
        self.change_frame(project, &mut annotation)?;
        if annotation == state.annotations[index] {
            return Ok(Diff::default());
        }
        state.annotations[index] = annotation;
        project.set_state::<Annotations>(&state)?;
        Ok(Diff::changed(self.id))
    }
}

impl Op for AnnotSet {
    const NAME: &'static str = "annot set";
    const ABOUT: &'static str =
        "Изменить аннотацию: границы по времени, положение, размер, поворот (с --at — в ключе)";
    const EXAMPLE: &'static str = "annot set a3 --from 4 --to 6.5 --x 0.2 --w 0.4 --rotation 15";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnotRm {
    pub id: AnnotId,
}

impl Command for AnnotRm {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Annotations>()?;
        let index = position(&state, self.id)?;
        state.annotations.remove(index);
        project.set_state::<Annotations>(&state)?;
        Ok(Diff::removed(self.id))
    }
}

impl Op for AnnotRm {
    const NAME: &'static str = "annot rm";
    const ABOUT: &'static str = "Удалить аннотацию";
    const EXAMPLE: &'static str = "annot rm a3";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

pub(crate) fn position(state: &AnnotationsState, id: AnnotId) -> Result<usize, CommandError> {
    index_of(&state.annotations, id, |a| a.id)
}

pub(crate) fn check_track(track: u32) -> Result<u32, CommandError> {
    if track < TRACKS {
        return Ok(track);
    }
    Err(CommandError::new(format!(
        "дорожка аннотаций должна быть от 0 до {}, а не {track}",
        TRACKS - 1
    )))
}

fn check_rect(rect: Rect) -> Result<Rect, CommandError> {
    let sides_ok = [rect.w(), rect.h()]
        .iter()
        .all(|side| (SIZE_MIN..=SIZE_MAX).contains(side));
    if !sides_ok {
        return Err(CommandError::new(format!(
            "ширина и высота аннотации должны быть от {SIZE_MIN} до {SIZE_MAX} (доли кадра), а не {}×{}",
            rect.w(),
            rect.h()
        )));
    }
    let corner_ok = [rect.x(), rect.y()]
        .iter()
        .all(|v| v.abs() <= POSITION_LIMIT);
    if !corner_ok {
        return Err(CommandError::new(format!(
            "угол аннотации ({}, {}) слишком далеко от кадра: x и y — доли кадра от -{POSITION_LIMIT} до {POSITION_LIMIT}",
            rect.x(),
            rect.y()
        )));
    }
    Ok(rect)
}

fn check_rotation(degrees: f64) -> Result<f64, CommandError> {
    if degrees.abs() <= ROTATION_LIMIT {
        return Ok(degrees);
    }
    Err(CommandError::new(format!(
        "поворот должен быть от -{ROTATION_LIMIT} до {ROTATION_LIMIT} градусов, а не {degrees}"
    )))
}
