//! Annotations in the scene: those of the shown recording moment, bottom to top in their layer
//! order (`AnnotationsState::shown_at`), each drawn by its kind's crate into `Band::Annotations`
//! under the camera. A kind without drawing yet (blur, 5.36) is skipped.

use std::collections::HashMap;

use dc_core::{OutputTime, TimeDelta, TimeMap};
use dc_fx_annotations::{AnnotId, Annotation, AnnotationsState, Kind, Placement, keyed, shown_for};
use dc_fx_drawing::DrawingBody;
use dc_fx_image::ImageBody;
use dc_fx_shape::ShapeBody;
use dc_fx_text::TextBody;
use dc_render::{Band, Layer, Scene, TextMeasure};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::evaluate::EvaluateError;

/// The annotations with their kinds' fields read once.
#[derive(Debug)]
pub(crate) struct Prepared {
    state: AnnotationsState,
    looks: HashMap<AnnotId, Look>,
}

#[derive(Debug)]
enum Look {
    Text(Fields<TextBody>),
    Shape(Fields<ShapeBody>),
    Image(Fields<ImageBody>),
    Drawing(Fields<DrawingBody>),
}

/// A kind's fields as typed values: the annotation's own and those of its keys, by key time.
#[derive(Debug)]
struct Fields<B> {
    base: B,
    keys: Vec<(TimeDelta, B)>,
}

impl<B: DeserializeOwned> Fields<B> {
    fn read(annotation: &Annotation) -> Result<Self, EvaluateError> {
        let read = |body: &Map<String, Value>| {
            serde_json::from_value(Value::Object(body.clone())).map_err(|error| {
                EvaluateError::Annotation {
                    id: annotation.id.to_string(),
                    error: error.to_string(),
                }
            })
        };
        let keys = annotation.keys.iter().map(|k| Ok((k.at, read(&k.body)?)));
        Ok(Self {
            base: read(&annotation.body)?,
            keys: keys.collect::<Result<_, EvaluateError>>()?,
        })
    }

    fn all(&self) -> impl Iterator<Item = &B> {
        std::iter::once(&self.base).chain(self.keys.iter().map(|(_, b)| b))
    }
}

impl Prepared {
    /// Reads the fields of every annotation that draws; a text's fonts must be in `measure`.
    pub(crate) fn new(
        state: AnnotationsState,
        measure: &TextMeasure,
    ) -> Result<Self, EvaluateError> {
        let mut looks = HashMap::new();
        for annotation in &state.annotations {
            if let Some(look) = look(annotation, measure)? {
                looks.insert(annotation.id, look);
            }
        }
        Ok(Self { state, looks })
    }

    /// Puts the annotations of output moment `t` into `scene`.
    pub(crate) fn evaluate(&self, map: &TimeMap, t: OutputTime, scene: &mut Scene) {
        let Some(segment) = map.segment_at(t) else {
            return;
        };
        let now = segment.source_at(t);
        let shown = self.state.shown_at(&segment.media, now);
        for annotation in shown {
            let Some(look) = self.looks.get(&annotation.id) else {
                continue;
            };
            let placement = Placement::at(annotation, scene.size, map, now, t);
            let offset = annotation.offset(now);
            let layer = match look {
                Look::Text(fields) => {
                    let body = keyed(&fields.base, &fields.keys, offset);
                    Layer::Text(dc_fx_text::evaluate(&body, &placement))
                }
                Look::Shape(fields) => {
                    let body = keyed(&fields.base, &fields.keys, offset);
                    Layer::Shapes(vec![dc_fx_shape::evaluate(&body, &placement, t)])
                }
                Look::Image(fields) => {
                    let body = keyed(&fields.base, &fields.keys, offset);
                    let elapsed = shown_for(annotation, map, now, t);
                    match dc_fx_image::evaluate(&body, &placement, elapsed) {
                        Some(sprite) => Layer::Sprite(sprite),
                        None => continue,
                    }
                }
                Look::Drawing(fields) => {
                    let body = keyed(&fields.base, &fields.keys, offset);
                    match dc_fx_drawing::evaluate(&body, &placement) {
                        Some(drawing) => Layer::Drawing(drawing),
                        None => continue,
                    }
                }
            };
            scene.push(Band::Annotations, layer);
        }
    }
}

fn look(annotation: &Annotation, measure: &TextMeasure) -> Result<Option<Look>, EvaluateError> {
    Ok(match annotation.kind {
        Kind::Text => {
            let fields = Fields::<TextBody>::read(annotation)?;
            for body in fields.all() {
                measure.check(&body.font)?;
            }
            Some(Look::Text(fields))
        }
        Kind::Shape => Some(Look::Shape(Fields::read(annotation)?)),
        Kind::Image => Some(Look::Image(Fields::read(annotation)?)),
        Kind::Drawing => Some(Look::Drawing(Fields::read(annotation)?)),
        Kind::Blur => None,
    })
}
