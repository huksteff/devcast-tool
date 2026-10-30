//! `cursor`: look and motion of the cursor of every recording.

use dc_core::{Changes, Command, CommandError, Diff, Op, Project, TimeDelta, within};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Cursor;
use crate::state::{
    BLUR_MAX, BOUNCE_MAX, BOUNCE_TIME_MAX, BOUNCE_TIME_MIN, CursorSettings, SIZE_MAX, SIZE_MIN,
    SWAY_MAX, Smoothing,
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CursorSet {
    /// Draw the cursor; click effects stay either way.
    pub show: Option<bool>,
    /// Size, 0.5..10: 1 is a 28 px arrow in a clip 1920 px wide.
    pub size: Option<f64>,
    /// exact — as recorded; focused or smooth — the spring of that motion preset.
    pub smoothing: Option<Smoothing>,
    /// Motion blur, 0..2; 0 turns it off.
    pub blur: Option<f64>,
    /// Squeeze on a click, 0..5; 0 turns it off.
    pub bounce: Option<f64>,
    /// Length of the squeeze, seconds (0.06..0.5).
    pub bounce_time: Option<f64>,
    /// Tilt in the direction of motion, 0..2; 0 turns it off.
    pub sway: Option<f64>,
    /// The last 0.67 s bring the cursor back to where it starts.
    #[serde(rename = "loop")]
    pub looped: Option<bool>,
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
        new.smoothing = self.smoothing.unwrap_or(new.smoothing);
        new.looped = self.looped.unwrap_or(new.looped);
        if let Some(size) = self.size {
            new.size = within("размер курсора", size, SIZE_MIN, SIZE_MAX)?;
        }
        if let Some(blur) = self.blur {
            new.blur = within("motion blur курсора", blur, 0.0, BLUR_MAX)?;
        }
        if let Some(bounce) = self.bounce {
            new.bounce = within("отскок при клике", bounce, 0.0, BOUNCE_MAX)?;
        }
        if let Some(secs) = self.bounce_time {
            let (min, max) = (BOUNCE_TIME_MIN.as_secs_f64(), BOUNCE_TIME_MAX.as_secs_f64());
            new.bounce_time =
                TimeDelta::from_secs_f64(within("время отскока (с)", secs, min, max)?);
        }
        if let Some(sway) = self.sway {
            new.sway = within("покачивание", sway, 0.0, SWAY_MAX)?;
        }
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
        "Курсор всех записей: показ, размер, сглаживание, blur, отскок, покачивание, loop";
    const EXAMPLE: &'static str = "cursor --size 3 --smoothing exact --sway 0";
    const SETTINGS: Option<&'static str> = Some("cursor");
}

/// `курсор: размер 2.50→3.00, сглаживание focused→exact`.
fn changes(old: &CursorSettings, new: &CursorSettings) -> String {
    let secs = |t: TimeDelta| t.as_secs_f64();
    let mut changes = Changes::default();
    changes
        .part(old.show != new.show, || {
            if new.show {
                "показан"
            } else {
                "скрыт"
            }
            .to_owned()
        })
        .number("размер", old.size, new.size, 2)
        .value("сглаживание", old.smoothing.name(), new.smoothing.name())
        .number("blur", old.blur, new.blur, 2)
        .number("отскок", old.bounce, new.bounce, 2)
        .number(
            "время отскока (с)",
            secs(old.bounce_time),
            secs(new.bounce_time),
            2,
        )
        .number("покачивание", old.sway, new.sway, 2)
        .switch("loop", old.looped, new.looped);
    format!("курсор: {}", changes.join())
}
