//! `cursor`: look and motion of the cursor of every recording.

use dc_core::{Command, CommandError, Diff, Op, Project};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Cursor;
use crate::state::{CursorSettings, SIZE_MAX, SIZE_MIN, Smoothing};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CursorSet {
    /// Draw the cursor; click effects stay either way.
    pub show: Option<bool>,
    /// Size, 0.5..10: 1 is a 28 px arrow in a clip 1920 px wide.
    pub size: Option<f64>,
    /// exact — as recorded; focused or smooth — the spring of that motion preset.
    pub smoothing: Option<Smoothing>,
}

impl Command for CursorSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Cursor>()?;
        let old = state.settings.clone();
        let new = &mut state.settings;
        new.show = self.show.unwrap_or(new.show);
        if let Some(size) = self.size {
            if !(SIZE_MIN..=SIZE_MAX).contains(&size) {
                return Err(CommandError::new(format!(
                    "размер курсора должен быть от {SIZE_MIN} до {SIZE_MAX}, а не {size}"
                )));
            }
            new.size = size;
        }
        new.smoothing = self.smoothing.unwrap_or(new.smoothing);
        if *new == old {
            return Ok(Diff::default());
        }
        let summary = changes(&old, new);
        project.set_state::<Cursor>(&state)?;
        Ok(Diff::note(summary))
    }
}

impl Op for CursorSet {
    const NAME: &'static str = "cursor";
    const ABOUT: &'static str =
        "Курсор всех записей: показать или скрыть, размер, сглаживание exact/focused/smooth";
    const EXAMPLE: &'static str = "cursor --size 3 --smoothing exact";
}

/// `курсор: размер 2.50→3.00, сглаживание focused→exact`.
fn changes(old: &CursorSettings, new: &CursorSettings) -> String {
    let switch = |on: bool| if on { "показан" } else { "скрыт" };
    let mut parts = Vec::new();
    if old.show != new.show {
        parts.push(switch(new.show).to_owned());
    }
    if old.size != new.size {
        parts.push(format!("размер {:.2}→{:.2}", old.size, new.size));
    }
    if old.smoothing != new.smoothing {
        parts.push(format!(
            "сглаживание {}→{}",
            old.smoothing.name(),
            new.smoothing.name()
        ));
    }
    format!("курсор: {}", parts.join(", "))
}
