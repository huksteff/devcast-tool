//! `captions paste`: a copy of a cue at another time, its words stretched with it.

use dc_core::time::Output;
use dc_core::{Command, CommandError, Diff, Op, Project, TimeArg, TimeDelta};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::{CaptionsEffect, Cue};
use crate::retime::{range, rescale};
use crate::span::Timeline;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptionsPaste {
    /// The cue to copy as `project.json` keeps it: its words come along; its id, recording and
    /// time are replaced.
    pub item: Cue,
    /// Start on the output video; the copy belongs to the recording shown there.
    pub at: TimeArg<Output>,
    /// Length, seconds of the output video; the words stretch with the cue.
    pub duration: f64,
}

impl Command for CaptionsPaste {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let length = TimeDelta::from_secs_f64(self.duration);
        if !self.duration.is_finite() || length <= TimeDelta::ZERO {
            return Err(CommandError::new(format!(
                "длительность реплики должна быть больше нуля (секунды), а не {}",
                self.duration
            )));
        }
        let timeline = Timeline::new(project);
        let mut state = project.state::<CaptionsEffect>()?;
        let mut cue = Cue {
            id: state.ids.allocate(),
            media: timeline.media_at(self.at.0)?,
            ..self.item.clone()
        };
        let start = timeline.start(&cue, self.at.0)?;
        let end = timeline.end(&cue, self.at.0 + length)?;
        if end <= start {
            return Err(CommandError::new(format!(
                "реплика {}–{} не помещается в одну запись: вставь её раньше или сделай короче",
                self.at.0,
                self.at.0 + length
            )));
        }
        let to = range(start, end)?;
        cue.words = rescale(&self.item.words, self.item.range, to)?;
        cue.range = to;
        cue.check()?;
        let id = cue.id;
        state.cues.push(cue);
        state.sort();
        project.set_state::<CaptionsEffect>(&state)?;
        Ok(Diff::added(id))
    }
}

impl Op for CaptionsPaste {
    const NAME: &'static str = "captions paste";
    const ABOUT: &'static str = "Вставить копию реплики (её JSON из project.json) в другое время: слова растягиваются с ней";
    const EXAMPLE: &'static str = r#"captions paste --item '{"id":"s1","media":"media/c1.mp4","range":{"start":0,"end":1500000},"words":[{"text":"Привет","range":{"start":0,"end":1500000}}]}' --at 12 --duration 1.5"#;
}
