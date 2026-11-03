//! Auto zooms from the clicks of the recordings (`dc_fx_zoom::suggest`), one path for
//! `devcast zoom suggest`, `devcast zoom add --at clicks`, the zooms after a recording (of
//! `devcast record` and of the app) and the editor's «Подобрать зумы». The suggestion is not a
//! registry command: a command is a pure function of the project, and the clicks lie in the
//! recordings' telemetry files, which the caller's project folder gives. The step
//! `zoom add --at clicks` ([`by_clicks`]) asks for them; [`expand`] builds it.

use std::path::Path;

use dc_capture::input::{Button, CursorEvent};
use dc_capture::project::{Recorded, fraction, recorded};
use dc_core::{Command, Op, Project};
use dc_fx_zoom::{
    Action, DEFAULT_SCALE, Mode, Pointer, Recording, Suggestions, ZoomAdd, ZoomEffect,
};
use dc_project::history_log::Event;
use dc_project::{HistoryLog, ProjectDir, ProjectLock, Working};
use serde::Deserialize;
use serde_json::Value;

use crate::apply::{Applied, apply_commands, as_commands};
use crate::error::OpsError;
use crate::plan::{Commands, Step};
pub use dc_fx_zoom::{AT_CLICKS as CLICKS, by_clicks};

/// What `zoom add --at clicks` takes besides `--at`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClickArgs {
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default)]
    mode: Mode,
}

fn default_scale() -> f64 {
    DEFAULT_SCALE
}

/// `zoom add --at clicks` of the project at `root` (one op, a step of a plan, a button of the
/// editor): the suggested zooms, their reason in `notes` when there are none; `None` for any
/// other step, which the registry builds.
pub fn expand(
    root: &Path,
    project: &Project,
    step: &Step,
    notes: &mut Vec<String>,
) -> Result<Option<Commands>, OpsError> {
    if step.op != ZoomAdd::NAME || step.args.get("at") != Some(&Value::from(CLICKS)) {
        return Ok(None);
    }
    let mut fields = step.args.clone();
    fields.remove("at");
    let args: ClickArgs = serde_json::from_value(Value::Object(fields)).map_err(|e| {
        OpsError::Usage(format!(
            "zoom add --at clicks: {e}. С --at clicks задаются только --scale и --mode: \
             длительность и фокус берутся из кликов"
        ))
    })?;
    let found = click_zooms(root, project)?;
    notes.extend(no_click_zooms(&found));
    let commands = click_commands(&found, args.scale, args.mode);
    Ok(Some(
        commands
            .into_iter()
            .map(|c| Box::new(c) as Box<dyn Command>)
            .collect(),
    ))
}

/// The zooms the clicks of the recordings of the project at `root` suggest.
pub fn click_zooms(root: &Path, project: &Project) -> Result<Suggestions, OpsError> {
    let recorded = recorded(root, project).map_err(|e| OpsError::Failed(e.to_string()))?;
    let recordings: Vec<Recording> = recorded.iter().map(pointer).collect();
    dc_fx_zoom::suggest(project, &recordings).map_err(|e| OpsError::Failed(e.to_string()))
}

/// The `zoom add` of every suggestion with `scale` and `mode`; applied as one undo step named
/// [`ZoomAdd`]'s name.
pub fn click_commands(found: &Suggestions, scale: f64, mode: Mode) -> Vec<ZoomAdd> {
    found.zooms.iter().map(|z| z.command(scale, mode)).collect()
}

/// Why there is nothing to suggest, or `None` if there is something.
pub fn no_click_zooms(found: &Suggestions) -> Option<String> {
    if !found.zooms.is_empty() {
        return None;
    }
    Some(match (found.clicks, found.taken) {
        (0, _) if found.hidden > 0 => {
            format!(
                "автозумов нет: все клики ({}) вырезаны или вне кадра",
                found.hidden
            )
        }
        (0, _) => "автозумов нет: в записях нет кликов".to_owned(),
        _ => format!(
            "автозумов нет: у всех групп кликов ({}) уже есть зумы",
            found.taken
        ),
    })
}

/// A recording was just added to the project `dir` (the stop of `devcast record` or of the
/// app's recording, a killed recording recovered on open): if the zoom setting `auto` is on, the
/// suggested zooms as one undo step, written as a `devcast` edit is (`history.jsonl`, then
/// `session.json` when unsaved changes are left, otherwise `project.json`). `None` if the setting
/// is off. The caller holds `_lock` of `dir`.
pub fn after_recording(dir: &ProjectDir, _lock: &ProjectLock) -> Result<Option<Applied>, OpsError> {
    let failed = |e: dc_project::ProjectError| OpsError::Failed(e.to_string());
    let mut working = Working::load(dir, false).map_err(failed)?;
    let settings = working
        .project
        .state::<ZoomEffect>()
        .map_err(|e| OpsError::Failed(e.to_string()))?
        .settings;
    if !settings.auto {
        return Ok(None);
    }
    let found = click_zooms(dir.root(), &working.project)?;
    let commands = click_commands(&found, DEFAULT_SCALE, Mode::default());
    let applied = apply_commands(
        &mut working.project,
        &mut working.history,
        ZoomAdd::NAME,
        &as_commands(&commands),
        false,
    )?;
    if applied.changed {
        let change = working.history.changes().last().cloned().ok_or_else(|| {
            OpsError::Failed(
                "внутренняя ошибка: автозумы не попали в историю. Это баг, сообщи о нём".to_owned(),
            )
        })?;
        HistoryLog::new(dir)
            .append(&[Event::Do(change)])
            .map_err(failed)?;
        match working.unsaved {
            true => dir.save_session(&working.project),
            false => dir.save(&working.project),
        }
        .map_err(failed)?;
    }
    Ok(Some(applied))
}

/// The moves, releases and left, right or middle presses of one recording, in time order.
fn pointer(r: &Recorded<'_>) -> Recording {
    let mut pointer: Vec<Pointer> = r
        .telemetry
        .cursor
        .iter()
        .filter_map(|e| {
            let (action, (x, y)) = match e.event {
                CursorEvent::Move { x, y } => (Action::Move, (x, y)),
                CursorEvent::Up { x, y, .. } => (Action::Release, (x, y)),
                _ => return None,
            };
            Some(Pointer {
                at: e.at,
                point: fraction(r.media, x, y),
                action,
            })
        })
        .collect();
    let presses = r
        .telemetry
        .clicks()
        .into_iter()
        .filter(|c| matches!(c.button, Button::Left | Button::Right | Button::Middle));
    pointer.extend(presses.map(|c| Pointer {
        at: c.at,
        point: fraction(r.media, c.x, c.y),
        action: Action::Press { double: c.double },
    }));
    pointer.sort_by_key(|p| p.at);
    Recording {
        media: r.media.path.clone(),
        pointer,
    }
}
