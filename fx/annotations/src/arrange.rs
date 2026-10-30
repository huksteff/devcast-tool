//! `annot move` and `annot layer`: where an annotation lies on the timeline and above which
//! annotations it is painted.

use dc_core::time::Output;
use dc_core::{Command, CommandError, Diff, Op, Project, TimeArg};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::commands::{check_track, position};
use crate::model::{AnnotId, Annotation, Annotations};
use crate::span::{shown, span, timeline};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnotMove {
    pub id: AnnotId,
    /// New start on the output video; the length on the output video stays.
    pub at: Option<TimeArg<Output>>,
    /// New track, 0..31; there the annotation lies above the others.
    pub track: Option<u32>,
}

impl Command for AnnotMove {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        if self.at.is_none() && self.track.is_none() {
            return Err(CommandError::new(
                "куда двигать? Укажи --at (новое начало) и/или --track (дорожка)",
            ));
        }
        let mut state = project.state::<Annotations>()?;
        let index = position(&state, self.id)?;
        let mut annotation = state.annotations[index].clone();
        if let Some(TimeArg(at)) = self.at {
            let timeline = timeline(project)?;
            let shown = shown(&timeline, &annotation).ok_or_else(|| {
                CommandError::new(format!(
                    "{} не видна в готовом видео (её место вырезано): задай время через annot set {} --from … --to …",
                    self.id, self.id
                ))
            })?;
            let start = timeline.start(at)?;
            let end = timeline.end(at + shown.duration())?;
            (annotation.media, annotation.range) = span(&timeline, start, end)?;
        }
        let track = self.track.map(check_track).transpose()?;
        let new_track = track.filter(|t| *t != annotation.track);
        annotation.track = track.unwrap_or(annotation.track);
        if annotation == state.annotations[index] {
            return Ok(Diff::default());
        }
        if new_track.is_some() {
            state.annotations.remove(index);
            state.annotations.push(annotation);
        } else {
            state.annotations[index] = annotation;
        }
        project.set_state::<Annotations>(&state)?;
        Ok(Diff::changed(self.id))
    }
}

impl Op for AnnotMove {
    const NAME: &'static str = "annot move";
    const ABOUT: &'static str =
        "Передвинуть аннотацию по времени (длина сохраняется) и/или на другую дорожку";
    const EXAMPLE: &'static str = "annot move a3 --at 12 --track 1";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

/// Where to put an annotation among the others of its track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// Above the next annotation of the track that is on screen at the same time.
    Up,
    /// Under the previous annotation of the track that is on screen at the same time.
    Down,
    /// Above every annotation of the track.
    Top,
    /// Under every annotation of the track.
    Bottom,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnotLayer {
    pub id: AnnotId,
    pub step: Step,
}

impl Command for AnnotLayer {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Annotations>()?;
        let index = position(&state, self.id)?;
        let list = &state.annotations;
        let this = &list[index];
        let peer = |j: &usize| list[*j].track == this.track;
        let meets = |j: &usize| peer(j) && this.meets(&list[*j]);
        let target = match self.step {
            Step::Up => (index + 1..list.len()).find(meets),
            Step::Down => (0..index).rev().find(meets),
            Step::Top => (index + 1..list.len()).rfind(peer),
            Step::Bottom => (0..index).find(peer),
        };
        let Some(target) = target else {
            return Ok(Diff::default());
        };
        let annotation: Annotation = state.annotations.remove(index);
        state.annotations.insert(target, annotation);
        project.set_state::<Annotations>(&state)?;
        Ok(Diff::changed(self.id))
    }
}

impl Op for AnnotLayer {
    const NAME: &'static str = "annot layer";
    const ABOUT: &'static str = "Поднять или опустить аннотацию среди аннотаций её дорожки: up, down, top, bottom (дорожка выше — слой выше)";
    const EXAMPLE: &'static str = "annot layer a3 top";
    const POSITIONAL: &'static [&'static str] = &["id", "step"];
}
