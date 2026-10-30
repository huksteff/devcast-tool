//! `blur set`: how strong a blur annotation is, its tint and its corners.

use dc_core::time::Output;
use dc_core::{Command, CommandError, Diff, Op, Project, TimeArg};
use dc_fx_annotations::{AnnotId, Annotations, Kind, edit_body, key_offset};
use dc_render::Rgba;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::{BlurBody, RADIUS_MAX, STRENGTH_MAX, STRENGTH_MIN};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BlurSet {
    pub id: AnnotId,
    /// Strength: σ of the blur, px when the short side of the frame is 1080 px, 1..100.
    pub strength: Option<f64>,
    /// Tint, #rrggbbaa; the alpha is how much it covers: 00 a pure blur, ff a solid fill.
    pub tint: Option<Rgba>,
    /// Corner radius, px when the short side of the frame is 1080 px, 0..540.
    pub radius: Option<f64>,
    /// Moment on the output video: the fields change in the key there (created with what is
    /// shown). Without it they change in the annotation and in every key.
    pub at: Option<TimeArg<Output>>,
}

impl BlurSet {
    /// `old` with the given fields replaced, each checked.
    fn change(&self, old: &BlurBody) -> Result<BlurBody, CommandError> {
        let mut new = old.clone();
        if let Some(strength) = self.strength {
            new.strength = within("сила размытия", strength, STRENGTH_MIN, STRENGTH_MAX)?;
        }
        if let Some(radius) = self.radius {
            new.radius = within("радиус углов", radius, 0.0, RADIUS_MAX)?;
        }
        new.tint = self.tint.unwrap_or(new.tint);
        Ok(new)
    }
}

impl Command for BlurSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Annotations>()?;
        let ids: Vec<AnnotId> = state.annot_ids().collect();
        let annotation = state
            .annotations
            .iter_mut()
            .find(|a| a.id == self.id)
            .ok_or_else(|| CommandError::not_found(self.id, ids))?;
        if annotation.kind != Kind::Blur {
            return Err(CommandError::new(format!(
                "{} — не размытие: blur set меняет только размытия (devcast ls annots покажет виды)",
                self.id
            )));
        }
        let at = match self.at {
            Some(TimeArg(t)) => Some(key_offset(project, annotation, t)?),
            None => None,
        };
        if !edit_body(annotation, at, |old: &BlurBody| self.change(old))? {
            return Ok(Diff::default());
        }
        project.set_state::<Annotations>(&state)?;
        Ok(Diff::changed(self.id))
    }
}

impl Op for BlurSet {
    const NAME: &'static str = "blur set";
    const ABOUT: &'static str = "Изменить размытие: силу, тонировку (альфа — от чистого размытия до сплошной заливки), радиус углов (с --at — в ключе)";
    const EXAMPLE: &'static str = "blur set a2 --strength 30 --tint \"#00000080\" --radius 16";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

fn within(what: &str, value: f64, min: f64, max: f64) -> Result<f64, CommandError> {
    if (min..=max).contains(&value) {
        return Ok(value);
    }
    Err(CommandError::new(format!(
        "{what}: нужно от {min} до {max}, а не {value}"
    )))
}
