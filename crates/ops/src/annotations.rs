//! Annotations in the scene: those of the shown recording moment, bottom to top in their layer
//! order (`AnnotationsState::shown_at`), each drawn by its kind's crate into `Band::Annotations`
//! under the camera. A kind without drawing yet (5.33–5.36) is skipped.

use std::collections::HashMap;

use dc_core::{OutputTime, TimeMap};
use dc_fx_annotations::{AnnotId, Annotation, AnnotationsState, Kind, Placement};
use dc_fx_text::TextBody;
use dc_render::{Band, Layer, Scene, TextMeasure};

use crate::evaluate::EvaluateError;

/// The annotations with their kinds' fields read once.
#[derive(Debug)]
pub(crate) struct Prepared {
    state: AnnotationsState,
    looks: HashMap<AnnotId, Look>,
}

#[derive(Debug)]
enum Look {
    Text(TextBody),
}

impl Prepared {
    /// Reads the fields of every annotation that draws; a text's font must be in `measure`.
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
        let shown = self.state.shown_at(&segment.media, segment.source_at(t));
        for annotation in shown {
            if let Some(Look::Text(body)) = self.looks.get(&annotation.id) {
                let text = dc_fx_text::evaluate(body, &Placement::of(annotation, scene.size));
                scene.push(Band::Annotations, Layer::Text(text));
            }
        }
    }
}

fn look(annotation: &Annotation, measure: &TextMeasure) -> Result<Option<Look>, EvaluateError> {
    let broken = |error: serde_json::Error| EvaluateError::Annotation {
        id: annotation.id.to_string(),
        error: error.to_string(),
    };
    Ok(match annotation.kind {
        Kind::Text => {
            let body: TextBody = annotation.body().map_err(broken)?;
            measure.check(&body.font)?;
            Some(Look::Text(body))
        }
        Kind::Shape | Kind::Image | Kind::Drawing | Kind::Blur => None,
    })
}
