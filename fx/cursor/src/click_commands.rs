//! `clicks`: the effect of every click; `click set` / `click reset`: one click otherwise.

use dc_core::time::Output;
use dc_core::{
    Command, CommandError, Diff, Op, Project, SourceTime, TimeArg, TimeDelta, TimeMap, within,
};
use dc_render::Rgba;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Cursor;
use crate::click_state::{
    CLICK_SCALE_MAX, CLICK_SCALE_MIN, CLICK_TIME_MAX, CLICK_TIME_MIN, ClickChange, ClickKind,
    ClickStyle,
};

/// Fields shared by `clicks` and `click set`, checked.
struct ClickArgs {
    style: Option<ClickKind>,
    color: Option<Rgba>,
    scale: Option<f64>,
    opacity: Option<f64>,
    duration: Option<f64>,
}

impl ClickArgs {
    fn checked(self) -> Result<Self, CommandError> {
        if let Some(scale) = self.scale {
            within(
                "размер эффекта клика",
                scale,
                CLICK_SCALE_MIN,
                CLICK_SCALE_MAX,
            )?;
        }
        if let Some(opacity) = self.opacity {
            within("прозрачность эффекта клика", opacity, 0.0, 1.0)?;
        }
        if let Some(secs) = self.duration {
            let (min, max) = (CLICK_TIME_MIN.as_secs_f64(), CLICK_TIME_MAX.as_secs_f64());
            within("длительность эффекта клика (с)", secs, min, max)?;
        }
        Ok(self)
    }

    fn duration(&self) -> Option<TimeDelta> {
        self.duration.map(TimeDelta::from_secs_f64)
    }
}

/// `ripple, #2563ebff, ×1.00, 100 %, 0.60 с`.
fn describe(style: &ClickStyle) -> String {
    format!(
        "{}, {}, ×{:.2}, {:.0} %, {:.2} с",
        style.kind.name(),
        style.color,
        style.scale,
        style.opacity * 100.0,
        style.duration.as_secs_f64()
    )
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClicksSet {
    /// off, ripple or flash.
    pub style: Option<ClickKind>,
    /// #rrggbbaa.
    pub color: Option<Rgba>,
    /// Size, 0.5..2.
    pub scale: Option<f64>,
    /// Opacity, 0..1.
    pub opacity: Option<f64>,
    /// Seconds, 0.12..1.2.
    pub duration: Option<f64>,
}

impl ClicksSet {
    /// Every argument as in `style`, for `devcast defaults show`.
    pub(crate) fn of(style: &ClickStyle) -> Self {
        Self {
            style: Some(style.kind),
            color: Some(style.color),
            scale: Some(style.scale),
            opacity: Some(style.opacity),
            duration: Some(style.duration.as_secs_f64()),
        }
    }
}

impl Command for ClicksSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let args = ClickArgs {
            style: self.style,
            color: self.color,
            scale: self.scale,
            opacity: self.opacity,
            duration: self.duration,
        }
        .checked()?;
        let mut state = project.state::<Cursor>()?;
        let old = state.settings.click.clone();
        let new = &mut state.settings.click;
        new.kind = args.style.unwrap_or(new.kind);
        new.color = args.color.unwrap_or(new.color);
        new.scale = args.scale.unwrap_or(new.scale);
        new.opacity = args.opacity.unwrap_or(new.opacity);
        new.duration = args.duration().unwrap_or(new.duration);
        if state.settings.click == old {
            return Ok(Diff::default());
        }
        let summary = format!("эффект кликов: {}", describe(&state.settings.click));
        project.set_state::<Cursor>(&state)?;
        Ok(Diff::note(summary))
    }
}

impl Op for ClicksSet {
    const NAME: &'static str = "clicks";
    const ABOUT: &'static str =
        "Эффект всех кликов: стиль off/ripple/flash, цвет, размер, прозрачность, длительность";
    const EXAMPLE: &'static str = "clicks --style ripple --color #ef4444ff --duration 0.8";
    const SETTINGS: Option<&'static str> = Some("clicks");
}

/// The press nearest to output moment `at`: its recording and moment there.
fn press(project: &Project, at: TimeArg<Output>) -> Result<(String, SourceTime), CommandError> {
    let map = TimeMap::build(project);
    let (clip, source) = map.to_source(at.0).ok_or_else(|| {
        CommandError::new(format!(
            "момент {} вне видео (длительность {:.3} с): возьми время клика из devcast events clicks",
            at.0.as_secs_f64(),
            map.duration().as_secs_f64()
        ))
    })?;
    let media = project
        .clip(clip)
        .map(|c| c.media.path.clone())
        .ok_or_else(|| CommandError::new(format!("клипа {clip} нет в проекте")))?;
    Ok((media, source))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClickSet {
    /// Output moment of the click, as `devcast events clicks` shows it.
    pub at: TimeArg<Output>,
    /// off, ripple or flash.
    pub style: Option<ClickKind>,
    /// #rrggbbaa.
    pub color: Option<Rgba>,
    /// Size, 0.5..2.
    pub scale: Option<f64>,
    /// Opacity, 0..1.
    pub opacity: Option<f64>,
    /// Seconds, 0.12..1.2.
    pub duration: Option<f64>,
}

impl Command for ClickSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let args = ClickArgs {
            style: self.style,
            color: self.color,
            scale: self.scale,
            opacity: self.opacity,
            duration: self.duration,
        }
        .checked()?;
        let (media, source) = press(project, self.at)?;
        let mut state = project.state::<Cursor>()?;
        let old = state.clicks.clone();
        let index = match state.clicks.iter().position(|c| c.is_for(&media, source)) {
            Some(index) => index,
            None => {
                state.clicks.push(ClickChange {
                    media,
                    at: source,
                    kind: None,
                    color: None,
                    scale: None,
                    opacity: None,
                    duration: None,
                });
                state.clicks.len() - 1
            }
        };
        let change = &mut state.clicks[index];
        change.kind = args.style.or(change.kind);
        change.color = args.color.or(change.color);
        change.scale = args.scale.or(change.scale);
        change.opacity = args.opacity.or(change.opacity);
        change.duration = args.duration().or(change.duration);
        if state.clicks == old {
            return Ok(Diff::default());
        }
        let summary = format!(
            "клик {:.3} с: {}",
            self.at.0.as_secs_f64(),
            describe(&state.clicks[index].applied_to(&state.settings.click))
        );
        project.set_state::<Cursor>(&state)?;
        Ok(Diff::note(summary))
    }
}

impl Op for ClickSet {
    const NAME: &'static str = "click set";
    const ABOUT: &'static str =
        "Один клик иначе остальных: --style off отключает его эффект, другие поля перенастраивают";
    const EXAMPLE: &'static str = "click set --at 12.4 --style off";
    const POSITIONAL: &'static [&'static str] = &["at"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClickReset {
    /// Output moment of the click.
    pub at: TimeArg<Output>,
}

impl Command for ClickReset {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let (media, source) = press(project, self.at)?;
        let mut state = project.state::<Cursor>()?;
        let before = state.clicks.len();
        state.clicks.retain(|c| !c.is_for(&media, source));
        if state.clicks.len() == before {
            return Err(CommandError::new(format!(
                "у клика {:.3} с нет своих настроек: он уже как все (devcast clicks)",
                self.at.0.as_secs_f64()
            )));
        }
        project.set_state::<Cursor>(&state)?;
        Ok(Diff::note(format!(
            "клик {:.3} с: как все",
            self.at.0.as_secs_f64()
        )))
    }
}

impl Op for ClickReset {
    const NAME: &'static str = "click reset";
    const ABOUT: &'static str = "Вернуть клику общий эффект кликов";
    const EXAMPLE: &'static str = "click reset --at 12.4";
    const POSITIONAL: &'static [&'static str] = &["at"];
}
