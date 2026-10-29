//! `keys`: the pressed-keys plate of every recording.

use dc_core::{Command, CommandError, Diff, Op, Project, TimeDelta};
use dc_render::Rgba;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Keys;
use crate::state::{DURATION_MAX, DURATION_MIN, KeysSettings, Position, SIZE_MAX, SIZE_MIN};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeysSet {
    /// Show the shortcuts pressed during the recordings (Ctrl+Shift+P, F5); off by default.
    pub show: Option<bool>,
    /// Where the plate stands: top_left, top, top_right, bottom_left, bottom, bottom_right.
    pub position: Option<Position>,
    /// Size, 0.5..3: 1 is a 36 px font when the short side of the frame is 1080 px.
    pub size: Option<f64>,
    /// Colour of the letters, #rrggbbaa.
    pub color: Option<Rgba>,
    /// Colour of the plate, #rrggbbaa.
    pub background: Option<Rgba>,
    /// Seconds a shortcut stays after its last press (0.3..5); pressed again meanwhile, it is
    /// counted: Ctrl+Z ×3.
    pub duration: Option<f64>,
}

impl Command for KeysSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Keys>()?;
        let old = state.settings.clone();
        let new = &mut state.settings;
        new.show = self.show.unwrap_or(new.show);
        new.position = self.position.unwrap_or(new.position);
        new.color = self.color.unwrap_or(new.color);
        new.background = self.background.unwrap_or(new.background);
        if let Some(size) = self.size {
            new.size = within("размер клавиш", size, SIZE_MIN, SIZE_MAX)?;
        }
        if let Some(secs) = self.duration {
            let (min, max) = (DURATION_MIN.as_secs_f64(), DURATION_MAX.as_secs_f64());
            new.duration = TimeDelta::from_secs_f64(within("время показа (с)", secs, min, max)?);
        }
        if *new == old {
            return Ok(Diff::default());
        }
        let summary = changes(&old, new);
        project.set_state::<Keys>(&state)?;
        Ok(Diff::note(summary))
    }
}

impl Op for KeysSet {
    const NAME: &'static str = "keys";
    const ABOUT: &'static str =
        "Показ нажатых клавиш записей: включить, позиция, размер, цвета, время показа";
    const EXAMPLE: &'static str = "keys --show --position bottom_right --size 1.5";
}

fn within(what: &str, value: f64, min: f64, max: f64) -> Result<f64, CommandError> {
    if (min..=max).contains(&value) {
        return Ok(value);
    }
    Err(CommandError::new(format!(
        "{what}: нужно от {min} до {max}, а не {value}"
    )))
}

/// `клавиши: показаны, позиция bottom→top_right, размер 1.00→1.50`.
fn changes(old: &KeysSettings, new: &KeysSettings) -> String {
    let number = |name: &str, a: f64, b: f64| (a != b).then(|| format!("{name} {a:.2}→{b:.2}"));
    let color = |name: &str, a: Rgba, b: Rgba| (a != b).then(|| format!("{name} {a}→{b}"));
    let parts = [
        (old.show != new.show).then(|| {
            if new.show {
                "показаны"
            } else {
                "скрыты"
            }
            .to_owned()
        }),
        (old.position != new.position)
            .then(|| format!("позиция {}→{}", old.position.name(), new.position.name())),
        number("размер", old.size, new.size),
        color("цвет", old.color, new.color),
        color("плашка", old.background, new.background),
        number(
            "время показа (с)",
            old.duration.as_secs_f64(),
            new.duration.as_secs_f64(),
        ),
    ];
    let parts: Vec<_> = parts.into_iter().flatten().collect();
    format!("клавиши: {}", parts.join(", "))
}
