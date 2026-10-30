//! `annot anim`: how an annotation comes in and goes out.

use dc_core::{Command, CommandError, Diff, Op, Project, TimeDelta};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::commands::position;
use crate::model::{AnnotId, Annotations, Kind};
use crate::motion::{Animation, STAGE_MAX, Stage};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnotAnim {
    pub id: AnnotId,
    /// Entrance: none, fade, pop, slide_left, slide_right, slide_top, slide_bottom (from that
    /// side), typewriter (text) or draw (shapes and drawings).
    pub enter: Option<Animation>,
    /// Length of the entrance, seconds of the output video, 0..10 (a sped-up part does not
    /// shorten it); 0.3 by default.
    pub enter_duration: Option<f64>,
    /// Exit, the same names: an exit is the entrance played backwards (slide_left leaves to the
    /// left, typewriter erases from the end).
    pub exit: Option<Animation>,
    /// Length of the exit, seconds of the output video, 0..10; 0.3 by default.
    pub exit_duration: Option<f64>,
}

impl Command for AnnotAnim {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Annotations>()?;
        let index = position(&state, self.id)?;
        let mut annotation = state.annotations[index].clone();
        let kind = annotation.kind;
        change(&mut annotation.enter, self.enter, self.enter_duration, kind)?;
        change(&mut annotation.exit, self.exit, self.exit_duration, kind)?;
        if annotation == state.annotations[index] {
            return Ok(Diff::default());
        }
        state.annotations[index] = annotation;
        project.set_state::<Annotations>(&state)?;
        Ok(Diff::changed(self.id))
    }
}

impl Op for AnnotAnim {
    const NAME: &'static str = "annot anim";
    const ABOUT: &'static str =
        "Анимация входа и выхода аннотации: fade, pop, slide_*, typewriter, draw";
    const EXAMPLE: &'static str =
        "annot anim a3 --enter slide_left --enter-duration 0.5 --exit fade";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

fn change(
    stage: &mut Stage,
    animation: Option<Animation>,
    seconds: Option<f64>,
    kind: Kind,
) -> Result<(), CommandError> {
    if let Some(animation) = animation {
        if !animation.fits(kind) {
            return Err(CommandError::new(format!(
                "анимация {} не подходит аннотации вида {}: typewriter есть только у текста, draw — у фигур и рисунков",
                animation.name(),
                kind.name()
            )));
        }
        stage.animation = animation;
    }
    if let Some(seconds) = seconds {
        let max = STAGE_MAX.as_secs_f64();
        if !(0.0..=max).contains(&seconds) {
            return Err(CommandError::new(format!(
                "длительность анимации должна быть от 0 до {max} с, а не {seconds}"
            )));
        }
        stage.duration = TimeDelta::from_secs_f64(seconds);
    }
    Ok(())
}
