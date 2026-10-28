//! `zoom motion`: camera motion of all zooms (presets and fine tuning).

use dc_core::{Command, CommandError, Diff, Op, Project, TimeDelta};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::{
    MOTION_BLUR_MAX, Preset, TRANSITION_MAX, TRANSITION_MIN, ZoomEffect, ZoomSettings,
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ZoomMotion {
    /// Applied first; the other arguments adjust it.
    pub preset: Option<Preset>,
    /// Camera smoothness: 0 sharp, 1 soft.
    pub smoothness: Option<f64>,
    /// Zoom-in duration, seconds (0.06..4).
    pub zoom_in: Option<f64>,
    /// Zoom-out duration, seconds (0.06..4).
    pub zoom_out: Option<f64>,
    /// Zoom motion blur, 0..2; 0 turns it off.
    pub blur: Option<f64>,
    /// Close zooms hand over without zooming out in between.
    pub connect: Option<bool>,
}

impl Command for ZoomMotion {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<ZoomEffect>()?;
        let old = state.settings.clone();
        let new = &mut state.settings;
        if let Some(preset) = self.preset {
            (new.zoom_in, new.zoom_out) = preset.durations();
        }
        if let Some(secs) = self.zoom_in {
            new.zoom_in = transition("вход", secs)?;
        }
        if let Some(secs) = self.zoom_out {
            new.zoom_out = transition("выход", secs)?;
        }
        if let Some(value) = self.smoothness {
            new.smoothness = within("плавность", value, 1.0)?;
        }
        if let Some(value) = self.blur {
            new.motion_blur = within("сила motion blur", value, MOTION_BLUR_MAX)?;
        }
        new.connect = self.connect.unwrap_or(new.connect);
        if *new == old {
            return Ok(Diff::default());
        }
        let summary = changes(&old, new);
        project.set_state::<ZoomEffect>(&state)?;
        Ok(Diff::note(summary))
    }
}

impl Op for ZoomMotion {
    const NAME: &'static str = "zoom motion";
    const ABOUT: &'static str =
        "Движение камеры всех зумов: пресет focused/smooth, плавность, вход и выход, blur, склейка";
    const EXAMPLE: &'static str = "zoom motion --preset smooth --smoothness 0.7";
}

fn transition(what: &str, secs: f64) -> Result<TimeDelta, CommandError> {
    let value = TimeDelta::from_secs_f64(secs);
    if (TRANSITION_MIN..=TRANSITION_MAX).contains(&value) {
        return Ok(value);
    }
    Err(CommandError::new(format!(
        "{what} зума должен длиться от {} до {} с, а не {secs}",
        TRANSITION_MIN.as_secs_f64(),
        TRANSITION_MAX.as_secs_f64()
    )))
}

fn within(what: &str, value: f64, max: f64) -> Result<f64, CommandError> {
    if (0.0..=max).contains(&value) {
        return Ok(value);
    }
    Err(CommandError::new(format!(
        "{what} должна быть от 0 до {max}, а не {value}"
    )))
}

/// `движение зума: вход 0.200→1.523 с, выход 0.200→1.015 с (Smooth)`.
fn changes(old: &ZoomSettings, new: &ZoomSettings) -> String {
    let secs = |t: TimeDelta| format!("{:.3}", t.as_secs_f64());
    let switch = |on: bool| if on { "вкл" } else { "выкл" };
    let mut parts = Vec::new();
    if old.zoom_in != new.zoom_in {
        parts.push(format!(
            "вход {}→{} с",
            secs(old.zoom_in),
            secs(new.zoom_in)
        ));
    }
    if old.zoom_out != new.zoom_out {
        parts.push(format!(
            "выход {}→{} с",
            secs(old.zoom_out),
            secs(new.zoom_out)
        ));
    }
    if old.smoothness != new.smoothness {
        parts.push(format!(
            "плавность {:.2}→{:.2}",
            old.smoothness, new.smoothness
        ));
    }
    if old.motion_blur != new.motion_blur {
        parts.push(format!(
            "motion blur {:.2}→{:.2}",
            old.motion_blur, new.motion_blur
        ));
    }
    if old.connect != new.connect {
        parts.push(format!(
            "склейка {}→{}",
            switch(old.connect),
            switch(new.connect)
        ));
    }
    let preset = new
        .preset()
        .map(|p| format!(" ({})", p.title()))
        .unwrap_or_default();
    format!("движение зума: {}{preset}", parts.join(", "))
}
