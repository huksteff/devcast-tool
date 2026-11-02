//! Auto zooms from the clicks of the recordings (`dc_ops::autozoom`): `devcast zoom suggest`
//! shows them, `zoom add --at clicks` (an op or a step of a plan) adds them in its undo step. The
//! zooms after a recording are `dc_ops::autozoom::after_recording`.

use dc_core::{Command, Op};
use dc_fx_zoom::{DEFAULT_SCALE, Mode, Suggestions, ZoomAdd};
use dc_ops::autozoom::CLICKS;
use dc_ops::{Commands, Step, click_commands, no_click_zooms};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::edit::{Ctx, answer};
use crate::error::CliError;
use crate::round::thousandths;
use crate::run::Answer;
use crate::workspace::Workspace;

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

fn suggestions(workspace: &Workspace) -> Result<Suggestions, CliError> {
    Ok(dc_ops::click_zooms(
        workspace.dir().root(),
        &workspace.project,
    )?)
}

/// `devcast zoom suggest`: the zooms `zoom add --at clicks` would add; changes nothing.
pub(crate) fn suggest(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let mut workspace = ctx.open(false)?;
    let found = suggestions(&workspace)?;
    let mut lines: Vec<String> = found
        .zooms
        .iter()
        .map(|z| {
            let (x, y) = (thousandths(z.focus.x), thousandths(z.focus.y));
            format!(
                "{}–{}  clicks={}  x={x}  y={y}",
                z.span.start(),
                z.span.end(),
                z.clicks
            )
        })
        .collect();
    lines.push(match no_click_zooms(&found) {
        Some(reason) => reason,
        None => format!("добавить все: devcast zoom add --at {CLICKS}"),
    });
    let items: Vec<Value> = found
        .zooms
        .iter()
        .map(|z| {
            json!({
                "from": z.span.start().as_secs_f64(), "to": z.span.end().as_secs_f64(),
                "clicks": z.clicks, "x": thousandths(z.focus.x), "y": thousandths(z.focus.y),
            })
        })
        .collect();
    let json = json!({
        "items": items, "clicks": found.clicks, "hidden": found.hidden, "taken": found.taken,
    });
    Ok(answer(&mut workspace, lines.join("\n"), json))
}

/// `zoom add --at clicks` (one op or a step of `apply plan.json`): the suggested zooms, their
/// reason under `warnings` when there are none; `None` for any other step.
pub(crate) fn clicks(
    workspace: &Workspace,
    step: &Step,
    warnings: &mut Vec<String>,
) -> Result<Option<Commands>, CliError> {
    if step.op != ZoomAdd::NAME || step.args.get("at") != Some(&Value::from(CLICKS)) {
        return Ok(None);
    }
    let mut fields = step.args.clone();
    fields.remove("at");
    let args: ClickArgs = serde_json::from_value(Value::Object(fields)).map_err(|e| {
        CliError::Usage(format!(
            "zoom add --at clicks: {e}. С --at clicks задаются только --scale и --mode:              длительность и фокус берутся из кликов"
        ))
    })?;
    let found = suggestions(workspace)?;
    warnings.extend(no_click_zooms(&found));
    let commands = click_commands(&found, args.scale, args.mode);
    Ok(Some(
        commands
            .into_iter()
            .map(|c| Box::new(c) as Box<dyn Command>)
            .collect(),
    ))
}
