//! `callout add`, `callout set`, `callout rm`: events of the callout track.

use dc_core::time::Output;
use dc_core::{Command, CommandError, Diff, Op, Project, SourceTime, TimeArg, TimeDelta, TimeMap};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::{Action, Callout, CalloutEvent, CalloutState, EventId};
use crate::text::check_text;

fn one_second() -> f64 {
    1.0
}

/// Action of an event as a command argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionName {
    /// Show the callout with the text of `callout style --text`.
    On,
    /// Type a new text.
    Text,
    /// Erase the text and hide the callout.
    Off,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalloutAdd {
    pub action: ActionName,
    /// Start on the output video.
    pub at: TimeArg<Output>,
    /// Text to type, only for `text`: one line, up to 80 characters.
    pub text: Option<String>,
    /// Typing or erasing time, seconds at normal speed even over a sped-up part.
    #[serde(default = "one_second")]
    pub duration: f64,
}

impl Command for CalloutAdd {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let (media, at) = moment(project, self.at.0)?;
        let action = action(self.action, self.text.clone())?;
        let duration = duration(self.duration)?;
        let mut state = project.state::<Callout>()?;
        let id = state.ids.allocate();
        state.events.push(CalloutEvent {
            id,
            media,
            at,
            duration,
            action,
        });
        project.set_state::<Callout>(&state)?;
        Ok(Diff::added(id))
    }
}

impl Op for CalloutAdd {
    const NAME: &'static str = "callout add";
    const ABOUT: &'static str = "Добавить событие облачка у курсора: on, text (печать текста) или off (время — по готовому видео)";
    const EXAMPLE: &'static str = "callout add text --at 3 --text \"Жми сюда\" --duration 1.5";
    const POSITIONAL: &'static [&'static str] = &["action"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalloutSet {
    pub id: EventId,
    pub action: Option<ActionName>,
    /// New start on the output video.
    pub at: Option<TimeArg<Output>>,
    /// New text, only for `text`: one line, up to 80 characters.
    pub text: Option<String>,
    /// Typing or erasing time, seconds at normal speed.
    pub duration: Option<f64>,
}

impl Command for CalloutSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Callout>()?;
        let index = position(&state, self.id)?;
        let mut event = state.events[index].clone();
        if let Some(TimeArg(t)) = self.at {
            (event.media, event.at) = moment(project, t)?;
        }
        if let Some(seconds) = self.duration {
            event.duration = duration(seconds)?;
        }
        if self.action.is_some() || self.text.is_some() {
            let name = self.action.unwrap_or(match event.action {
                Action::On => ActionName::On,
                Action::Text { .. } => ActionName::Text,
                Action::Off => ActionName::Off,
            });
            let text = self.text.clone().or_else(|| match &event.action {
                Action::Text { text } if name == ActionName::Text => Some(text.clone()),
                _ => None,
            });
            event.action = action(name, text)?;
        }
        if event == state.events[index] {
            return Ok(Diff::default());
        }
        state.events[index] = event;
        project.set_state::<Callout>(&state)?;
        Ok(Diff::changed(self.id))
    }
}

impl Op for CalloutSet {
    const NAME: &'static str = "callout set";
    const ABOUT: &'static str = "Изменить событие облачка: действие, время, текст, длительность";
    const EXAMPLE: &'static str = "callout set co2 --text \"Готово\" --duration 0.8";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalloutRm {
    pub id: EventId,
}

impl Command for CalloutRm {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Callout>()?;
        let index = position(&state, self.id)?;
        state.events.remove(index);
        project.set_state::<Callout>(&state)?;
        Ok(Diff::removed(self.id))
    }
}

impl Op for CalloutRm {
    const NAME: &'static str = "callout rm";
    const ABOUT: &'static str = "Удалить событие облачка";
    const EXAMPLE: &'static str = "callout rm co2";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

fn position(state: &CalloutState, id: EventId) -> Result<usize, CommandError> {
    state
        .events
        .iter()
        .position(|e| e.id == id)
        .ok_or_else(|| CommandError::not_found(id, state.event_ids()))
}

/// The recording and its source moment shown at output time `t`.
fn moment(project: &Project, t: dc_core::OutputTime) -> Result<(String, SourceTime), CommandError> {
    let map = TimeMap::build(project);
    if map.duration() <= TimeDelta::ZERO {
        return Err(CommandError::new(
            "в проекте нет клипов: сначала добавь видео (devcast clip add)",
        ));
    }
    let segment = map.segment_at(t).ok_or_else(|| {
        CommandError::new(format!(
            "время события {t} вне видео (0–{}): укажи время внутри",
            dc_core::OutputTime::ZERO + map.duration()
        ))
    })?;
    Ok((segment.media.clone(), segment.source_at(t)))
}

fn action(name: ActionName, text: Option<String>) -> Result<Action, CommandError> {
    match (name, text) {
        (ActionName::Text, Some(text)) => Ok(Action::Text {
            text: check_text(&text)?.to_owned(),
        }),
        (ActionName::Text, None) => Err(CommandError::new(
            "у события text нужен текст: добавь --text \"…\"",
        )),
        (_, Some(_)) => Err(CommandError::new(
            "текст бывает только у события text: убери --text или поставь --action text",
        )),
        (ActionName::On, None) => Ok(Action::On),
        (ActionName::Off, None) => Ok(Action::Off),
    }
}

fn duration(seconds: f64) -> Result<TimeDelta, CommandError> {
    let delta = TimeDelta::from_secs_f64(seconds);
    if seconds.is_finite() && delta > TimeDelta::ZERO {
        return Ok(delta);
    }
    Err(CommandError::new(format!(
        "длительность события должна быть больше нуля (секунды), а не {seconds}"
    )))
}
