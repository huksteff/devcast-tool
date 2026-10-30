//! `callout style`: look of every callout and the text of `on`.

use dc_core::{Command, CommandError, Diff, Op, Project, within};
use dc_render::{GradientColors, Rgba};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::{Callout, CalloutSettings, GLOW_MAX, SIZE_MAX, SIZE_MIN};
use crate::text::check_text;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalloutStyle {
    /// Text an `on` event shows, one line up to 80 characters; "" — wait for a `text` event.
    pub text: Option<String>,
    /// Bubble colour without a gradient, #rrggbbaa.
    pub background: Option<Rgba>,
    /// #rrggbbaa.
    pub text_color: Option<Rgba>,
    /// 0.5..2; 1 — the text 0.72 of the visible cursor height.
    pub size: Option<f64>,
    /// Fill the bubble with a flowing gradient of `colors`.
    pub gradient: Option<bool>,
    /// 2 to 4 gradient colours #rrggbbaa separated by commas.
    pub colors: Option<String>,
    /// Glow around the bubble, 0..40; 0 — none.
    pub glow: Option<f64>,
}

impl Command for CalloutStyle {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Callout>()?;
        let old = state.settings.clone();
        let new = &mut state.settings;
        if let Some(text) = &self.text {
            new.text = if text.trim().is_empty() {
                String::new()
            } else {
                check_text(text)?.to_owned()
            };
        }
        new.background = self.background.unwrap_or(new.background);
        new.text_color = self.text_color.unwrap_or(new.text_color);
        if let Some(size) = self.size {
            new.size = within("размер облачка", size, SIZE_MIN, SIZE_MAX)?;
        }
        new.gradient = self.gradient.unwrap_or(new.gradient);
        if let Some(colors) = &self.colors {
            new.colors = parse_colors(colors)?;
        }
        if let Some(glow) = self.glow {
            new.glow = within("свечение облачка", glow, 0.0, GLOW_MAX)?;
        }
        if *new == old {
            return Ok(Diff::default());
        }
        let summary = changes(&old, new);
        project.set_state::<Callout>(&state)?;
        Ok(Diff::note(summary))
    }
}

impl Op for CalloutStyle {
    const NAME: &'static str = "callout style";
    const ABOUT: &'static str =
        "Вид облачка у курсора: текст для on, цвета, размер, градиент 2–4 цвета, свечение";
    const EXAMPLE: &'static str =
        "callout style --gradient --colors \"#14b8c4ff,#a855f7ff\" --glow 14";
    const SETTINGS: Option<&'static str> = Some("callout");
}

fn parse_colors(text: &str) -> Result<GradientColors, CommandError> {
    let colors = text
        .split(',')
        .map(|c| c.trim().parse::<Rgba>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| CommandError::new(e.to_string()))?;
    GradientColors::try_from(colors).map_err(|e| {
        CommandError::new(format!(
            "{e}: перечисли их через запятую, например #14b8c4ff,#a855f7ff"
        ))
    })
}

/// `облачко: размер 1.00→1.50, градиент вкл`.
fn changes(old: &CalloutSettings, new: &CalloutSettings) -> String {
    let mut parts = Vec::new();
    if old.text != new.text {
        parts.push(format!("текст «{}»", new.text));
    }
    if old.background != new.background {
        parts.push(format!("фон {}", new.background));
    }
    if old.text_color != new.text_color {
        parts.push(format!("цвет текста {}", new.text_color));
    }
    if old.size != new.size {
        parts.push(format!("размер {:.2}→{:.2}", old.size, new.size));
    }
    if old.gradient != new.gradient {
        parts.push(format!(
            "градиент {}",
            if new.gradient { "вкл" } else { "выкл" }
        ));
    }
    if old.colors != new.colors {
        let colors: Vec<String> = new.colors.colors().iter().map(Rgba::to_string).collect();
        parts.push(format!("цвета {}", colors.join(",")));
    }
    if old.glow != new.glow {
        parts.push(format!("свечение {}→{}", old.glow, new.glow));
    }
    format!("облачко: {}", parts.join(", "))
}
