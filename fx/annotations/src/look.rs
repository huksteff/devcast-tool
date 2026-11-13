//! Annotations in the frame (`dc_render::look`): those of the shown recording moment, bottom to
//! top in their layer order (`AnnotationsState::shown_at`), into `Band::Annotations` under the
//! camera. This crate does not know its kinds' fields: each kind's crate draws them ([`Body`]),
//! and the registration hands the table of kinds to [`Picture`].

use std::collections::HashMap;
use std::fmt::Debug;

use dc_core::{OutputTime, SourceTime, TimeDelta, TimeMap};
use dc_render::look::{Frame, Inputs, Look, LookError, Prepare};
use dc_render::{Band, Layer, RenderError, TextMeasure};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::{AnnotId, Annotation, Annotations, AnnotationsState, Blend, Kind, Placement};
use crate::{keyed, shown_for};

/// A kind's fields as its crate draws them.
pub trait Body: DeserializeOwned + Blend + Clone + Debug + Send + Sync + 'static {
    /// The fields can be drawn with the fonts of `measure`.
    fn check(&self, measure: &TextMeasure) -> Result<(), RenderError> {
        let _ = measure;
        Ok(())
    }

    /// The layer of these fields placed at `placement`; none draws nothing.
    fn layer(&self, placement: &Placement, moment: &Moment<'_>) -> Option<Layer>;
}

/// The output moment an annotation is drawn at.
pub struct Moment<'a> {
    pub t: OutputTime,
    annotation: &'a Annotation,
    map: &'a TimeMap,
    now: SourceTime,
}

impl Moment<'_> {
    /// Output time since the annotation first showed (`shown_for`).
    pub fn shown_for(&self) -> TimeDelta {
        shown_for(self.annotation, self.map, self.now, self.t)
    }
}

/// How a kind reads an annotation's fields for drawing ([`read`] of its fields type).
pub type Read = fn(&Annotation, &TextMeasure) -> Result<Box<dyn Drawn>, LookError>;

/// An annotation's fields read once: they give its layer at any moment.
pub trait Drawn: Debug + Send + Sync {
    fn layer(&self, offset: TimeDelta, placement: &Placement, moment: &Moment<'_>)
    -> Option<Layer>;
}

/// The entry of the list of pictures (`dc_ops::Evaluator`) with the table of kinds: which kind
/// reads its fields how.
pub struct Picture(pub &'static [(Kind, Read)]);

/// An annotation's fields can not be read.
#[derive(Debug, thiserror::Error)]
#[error(
    "аннотация {id}: поля вида не читаются ({error}). Удали её (devcast annot rm {id}) и создай заново"
)]
pub struct BodyError {
    id: String,
    error: String,
}

/// A kind's fields as typed values: the annotation's own and those of its keys, by key time.
#[derive(Debug)]
struct Fields<B> {
    base: B,
    keys: Vec<(TimeDelta, B)>,
}

/// Reads the fields of `annotation` as `B` and checks they can be drawn.
pub fn read<B: Body>(
    annotation: &Annotation,
    measure: &TextMeasure,
) -> Result<Box<dyn Drawn>, LookError> {
    let read = |body: &Map<String, Value>| {
        serde_json::from_value::<B>(Value::Object(body.clone())).map_err(|error| {
            LookError::Effect(Box::new(BodyError {
                id: annotation.id.to_string(),
                error: error.to_string(),
            }))
        })
    };
    let keys = annotation.keys.iter().map(|k| Ok((k.at, read(&k.body)?)));
    let fields = Fields {
        base: read(&annotation.body)?,
        keys: keys.collect::<Result<_, LookError>>()?,
    };
    let all = std::iter::once(&fields.base).chain(fields.keys.iter().map(|(_, b)| b));
    for body in all {
        body.check(measure)?;
    }
    Ok(Box::new(fields))
}

impl<B: Body> Drawn for Fields<B> {
    fn layer(
        &self,
        offset: TimeDelta,
        placement: &Placement,
        moment: &Moment<'_>,
    ) -> Option<Layer> {
        keyed(&self.base, &self.keys, offset).layer(placement, moment)
    }
}

/// The annotations with their kinds' fields read once.
#[derive(Debug)]
struct Prepared {
    state: AnnotationsState,
    looks: HashMap<AnnotId, Box<dyn Drawn>>,
}

impl Prepare for Picture {
    fn prepare(&self, inputs: &mut Inputs<'_>) -> Result<Box<dyn Look>, LookError> {
        let state = inputs.project.state::<Annotations>()?;
        let mut looks = HashMap::new();
        for annotation in &state.annotations {
            let Some((_, read)) = self.0.iter().find(|(kind, _)| *kind == annotation.kind) else {
                continue;
            };
            looks.insert(annotation.id, read(annotation, inputs.measure)?);
        }
        Ok(Box::new(Prepared { state, looks }))
    }
}

impl Look for Prepared {
    fn draw(&self, frame: &mut Frame<'_>) {
        let (map, t) = (frame.map, frame.t);
        let Some(segment) = map.segment_at(t) else {
            return;
        };
        let now = segment.source_at(t);
        for annotation in self.state.shown_at(&segment.media, now) {
            let Some(look) = self.looks.get(&annotation.id) else {
                continue;
            };
            let placement = Placement::at(annotation, frame.scene.size, map, now, t);
            let moment = Moment {
                t,
                annotation,
                map,
                now,
            };
            if let Some(layer) = look.layer(annotation.offset(now), &placement, &moment) {
                frame.scene.push(Band::Annotations, layer);
            }
        }
    }
}
