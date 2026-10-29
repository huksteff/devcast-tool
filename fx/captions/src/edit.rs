//! `captions set`, `captions split`, `captions merge`, `captions rm`, `captions clear`.

use dc_core::time::Output;
use dc_core::{Command, CommandError, Diff, Op, Project, TimeArg};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::{CaptionsEffect, CueId};
use crate::retime::{range, rescale, retext};
use crate::span::Timeline;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptionsSet {
    pub id: CueId,
    /// New text; unchanged words keep their timing, new ones share the time between them.
    pub text: Option<String>,
    /// New start on the output video; words stretch with the cue.
    pub from: Option<TimeArg<Output>>,
    /// New end on the output video; words stretch with the cue.
    pub to: Option<TimeArg<Output>>,
}

impl Command for CaptionsSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<CaptionsEffect>()?;
        let index = state.position(self.id)?;
        let mut cue = state.cues[index].clone();
        if let Some(text) = &self.text {
            cue.words = retext(&cue.words, text)?;
        }
        if self.from.is_some() || self.to.is_some() {
            let timeline = Timeline::new(project);
            let start = match self.from {
                Some(TimeArg(t)) => timeline.start(&cue, t)?,
                None => cue.range.start(),
            };
            let end = match self.to {
                Some(TimeArg(t)) => timeline.end(&cue, t)?,
                None => cue.range.end(),
            };
            if end <= start {
                return Err(CommandError::new(format!(
                    "конец реплики {} должен быть позже начала",
                    self.id
                )));
            }
            let to = range(start, end)?;
            cue.words = rescale(&cue.words, cue.range, to)?;
            cue.range = to;
        }
        if cue == state.cues[index] {
            return Ok(Diff::default());
        }
        cue.check()?;
        state.cues[index] = cue;
        state.sort();
        project.set_state::<CaptionsEffect>(&state)?;
        Ok(Diff::changed(self.id))
    }
}

impl Op for CaptionsSet {
    const NAME: &'static str = "captions set";
    const ABOUT: &'static str = "Изменить реплику: текст и границы (время — по готовому видео)";
    const EXAMPLE: &'static str = "captions set s3 --from 12.5 --to 15";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptionsSplit {
    pub id: CueId,
    /// Moment on the output video; the cue splits at the pause between words nearest to it.
    pub at: TimeArg<Output>,
}

impl Command for CaptionsSplit {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<CaptionsEffect>()?;
        let index = state.position(self.id)?;
        let cue = state.cues[index].clone();
        let at = Timeline::new(project).start(&cue, self.at.0)?.micros();
        let gap = (1..cue.words.len()).min_by_key(|&i| {
            let middle =
                (cue.words[i - 1].range.end().micros() + cue.words[i].range.start().micros()) / 2;
            (middle - at).abs()
        });
        let Some(gap) = gap else {
            return Err(CommandError::new(format!(
                "в реплике {} одно слово: делить нечего",
                self.id
            )));
        };
        let mut left = cue.clone();
        let mut right = cue;
        right.id = state.ids.allocate();
        right.words = left.words.split_off(gap);
        left.range = range(left.range.start(), left.words[gap - 1].range.end())?;
        right.range = range(right.words[0].range.start(), right.range.end())?;
        left.check()?;
        right.check()?;
        let added = right.id;
        state.cues[index] = left;
        state.cues.push(right);
        state.sort();
        project.set_state::<CaptionsEffect>(&state)?;
        Ok(Diff::changed(self.id).and(Diff::added(added)))
    }
}

impl Op for CaptionsSplit {
    const NAME: &'static str = "captions split";
    const ABOUT: &'static str = "Разделить реплику на две в паузе между словами у момента --at";
    const EXAMPLE: &'static str = "captions split s3 --at 14.2";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptionsMerge {
    /// The cue that takes in the next cue of the same recording.
    pub id: CueId,
}

impl Command for CaptionsMerge {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<CaptionsEffect>()?;
        let index = state.position(self.id)?;
        let next = state
            .cues
            .get(index + 1)
            .filter(|c| c.media == state.cues[index].media)
            .ok_or_else(|| {
                CommandError::new(format!(
                    "после реплики {} в её записи нет следующей: объединять не с чем",
                    self.id
                ))
            })?
            .clone();
        let mut cue = state.cues[index].clone();
        let touching = (cue.words.last(), next.words.first());
        if let (Some(last), Some(first)) = touching
            && first.range.start() < last.range.end()
        {
            return Err(CommandError::new(format!(
                "слова реплик {} и {} перекрываются: сначала поправь их тайминг (captions set)",
                self.id, next.id
            )));
        }
        cue.range = range(cue.range.start(), cue.range.end().max(next.range.end()))?;
        cue.words.extend(next.words);
        cue.check()?;
        state.cues[index] = cue;
        state.cues.remove(index + 1);
        project.set_state::<CaptionsEffect>(&state)?;
        Ok(Diff::changed(self.id).and(Diff::removed(next.id)))
    }
}

impl Op for CaptionsMerge {
    const NAME: &'static str = "captions merge";
    const ABOUT: &'static str = "Объединить реплику со следующей";
    const EXAMPLE: &'static str = "captions merge s3";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptionsRm {
    pub id: CueId,
}

impl Command for CaptionsRm {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<CaptionsEffect>()?;
        let index = state.position(self.id)?;
        state.cues.remove(index);
        project.set_state::<CaptionsEffect>(&state)?;
        Ok(Diff::removed(self.id))
    }
}

impl Op for CaptionsRm {
    const NAME: &'static str = "captions rm";
    const ABOUT: &'static str = "Удалить реплику";
    const EXAMPLE: &'static str = "captions rm s3";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptionsClear {}

impl Command for CaptionsClear {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<CaptionsEffect>()?;
        let diff = state
            .cues
            .drain(..)
            .fold(Diff::default(), |diff, cue| diff.and(Diff::removed(cue.id)));
        project.set_state::<CaptionsEffect>(&state)?;
        Ok(diff)
    }
}

impl Op for CaptionsClear {
    const NAME: &'static str = "captions clear";
    const ABOUT: &'static str = "Удалить все реплики";
    const EXAMPLE: &'static str = "captions clear";
}
