//! Auto zooms from the clicks of the recordings (`dc_fx_zoom::suggest`): `devcast zoom suggest`
//! shows them, `devcast zoom add --at clicks` adds them as one undo step, and a recording added
//! to a project (`devcast record` stop, a killed one recovered on open) gets them when the
//! project's zoom setting `auto` is on.

use dc_capture::input::{Button, CursorEvent};
use dc_core::Op;
use dc_fx_zoom::{
    Action, DEFAULT_SCALE, Mode, Pointer, Recording, Suggestions, ZoomAdd, ZoomEffect,
};
use dc_ops::{Applied, Step, as_commands};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::edit::{self, Ctx, answer};
use crate::error::CliError;
use crate::round::thousandths;
use crate::run::Answer;
use crate::telemetry::{self, fraction};
use crate::workspace::Workspace;

/// The value of `zoom add --at` that places the suggested zooms.
const CLICKS: &str = "clicks";

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

/// The pointer of every recording on the track, for [`dc_fx_zoom::suggest`].
fn recordings(workspace: &Workspace) -> Result<Vec<Recording>, CliError> {
    let project = &workspace.project;
    let recorded = telemetry::read(workspace.dir().root(), project)?;
    Ok(recorded
        .iter()
        .map(|r| {
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
                    let point = fraction(r.media, x, y);
                    Some(Pointer {
                        at: e.at,
                        point,
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
        })
        .collect())
}

fn suggestions(workspace: &Workspace) -> Result<Suggestions, CliError> {
    let recordings = recordings(workspace)?;
    dc_fx_zoom::suggest(&workspace.project, &recordings)
        .map_err(|e| CliError::Failed(e.to_string()))
}

/// Why there is nothing to suggest, or `None` if there is something.
fn nothing(found: &Suggestions) -> Option<String> {
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
    lines.push(match nothing(&found) {
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

/// `devcast zoom …` from the registry; `zoom add --at clicks` adds the suggested zooms.
pub(crate) fn op(ctx: &Ctx<'_>, words: &[String]) -> Result<Answer, CliError> {
    let (info, taken) = ctx.registry.resolve(words)?;
    let mut fields = dc_ops::parse_args(info, &words[taken..])?;
    if info.name != ZoomAdd::NAME || fields.get("at") != Some(&Value::from(CLICKS)) {
        return edit::apply_steps(ctx, &[Step::new(info.name, fields)]);
    }
    fields.remove("at");
    let args: ClickArgs = serde_json::from_value(Value::Object(fields)).map_err(|e| {
        CliError::Usage(format!(
            "zoom add --at clicks: {e}. С --at clicks задаются только --scale и --mode: \
             длительность и фокус берутся из кликов"
        ))
    })?;
    let mut workspace = ctx.open(true)?;
    let found = suggestions(&workspace)?;
    let commands = commands(&found, &args);
    let mut answer =
        edit::apply_commands(ctx, &mut workspace, ZoomAdd::NAME, &as_commands(&commands))?;
    answer.warnings.extend(nothing(&found));
    Ok(answer)
}

fn commands(found: &Suggestions, args: &ClickArgs) -> Vec<ZoomAdd> {
    found
        .zooms
        .iter()
        .map(|z| z.command(args.scale, args.mode))
        .collect()
}

/// A recording was just added to the project of `workspace` (the stop of `devcast record`, or a
/// killed recording recovered on open): if the zoom setting `auto` is on, adds the suggested
/// zooms as one undo step and writes them. `None` if the setting is off.
pub(crate) fn after_recording(workspace: &mut Workspace) -> Result<Option<Applied>, CliError> {
    let settings = workspace
        .project
        .state::<ZoomEffect>()
        .map_err(|e| CliError::Failed(e.to_string()))?
        .settings;
    if !settings.auto {
        return Ok(None);
    }
    let found = suggestions(workspace)?;
    let args = ClickArgs {
        scale: DEFAULT_SCALE,
        mode: Mode::default(),
    };
    let commands = commands(&found, &args);
    let applied = dc_ops::apply_commands(
        &mut workspace.project,
        &mut workspace.history,
        ZoomAdd::NAME,
        &as_commands(&commands),
        false,
    )?;
    edit::write(workspace, &applied)?;
    Ok(Some(applied))
}
