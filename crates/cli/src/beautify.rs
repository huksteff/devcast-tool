//! `devcast beautify`: «Сделать красиво» from the console, the same run as the editor's
//! «Нейронка» tab (`dc-agent`): the user's agent edits the project in its folder with
//! `devcast` commands, all of them one undo step (`DEVCAST_GROUP`). The commands go to stderr
//! as they run; the answer is the agent's last words and how to go on with the conversation.

use std::time::{SystemTime, UNIX_EPOCH};

use clap::Args;
use dc_agent::{Ask, Event, Found, Job, Limits, Places, find, launch, model_name, pick, prompt};
use dc_settings::Agent;
use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::find::find_project;
use crate::recipes::agent_name;
use crate::run::Answer;

#[derive(Debug, Args)]
pub(crate) struct BeautifyArgs {
    #[arg(
        long = "recipe",
        help = "Рецепт по id или названию (можно несколько); без них — отмеченные (devcast recipes ls)"
    )]
    recipes: Vec<String>,
    #[arg(long, help = "Что ещё попросить, своими словами")]
    ask: Option<String>,
    #[arg(long, value_parser = ["claude", "codex"], help = "Нейронка на этот раз (иначе — devcast recipes agent)")]
    agent: Option<String>,
    #[arg(long, help = "Модель на этот раз (иначе — devcast recipes model)")]
    model: Option<String>,
    #[arg(
        long,
        requires = "ask",
        help = "Продолжить разговор: сессия из ответа прошлого запуска"
    )]
    resume: Option<String>,
}

pub(crate) fn beautify(ctx: &Ctx<'_>, args: &BeautifyArgs) -> Result<Answer, CliError> {
    let dir = find_project(ctx.globals.project.as_deref(), ctx.cwd)?;
    let ai = ctx.app_settings()?.ai;
    let ask = args.ask.as_deref().unwrap_or("");
    let request = match &args.resume {
        Some(_) => prompt::more(ask),
        None => prompt::first(&recipes(&ai, &args.recipes)?, ask),
    };
    let request = request.ok_or_else(|| {
        CliError::Usage(
            "просить нечего: отметь рецепты (devcast recipes check) или добавь --ask".to_owned(),
        )
    })?;
    let found;
    let agent = match ctx.local.agent {
        Some(agent) => agent,
        None => {
            found = find(&Places::here());
            let wanted = args.agent.as_deref().map(agent_of).or(ai.agent);
            pick(&found, wanted).ok_or_else(no_agent)?
        }
    };
    // A one-off provider override must not inherit the other provider's model alias.
    let saved_model = ai.model.as_deref().filter(|_| {
        args.agent
            .as_deref()
            .is_none_or(|name| ai.agent == Some(agent_of(name)))
    });
    let model = args.model.as_deref().or(saved_model);
    if ctx.globals.dry_run {
        let text = format!(
            "dry-run, нейронка не запущена: {}\n{request}",
            agent_name(agent.agent)
        );
        return Ok(Answer::new(
            text,
            json!({ "dry_run": true, "request": request }),
        ));
    }
    let system = dc_agent::system_file().map_err(|e| failed(&e))?;
    let exe = std::env::current_exe().map_err(|e| failed(&e))?;
    let devcast = exe.parent().unwrap_or(&exe).to_path_buf();
    let group = group_id();
    let ask = Ask {
        found: agent,
        model,
        project: dir.root(),
        devcast: &devcast,
        system: &system,
        request: &request,
        resume: args.resume.as_deref(),
        group: &group,
    };
    let job = Job::start(launch(&ask), || {}).map_err(|e| failed(&e))?;
    follow(ctx, agent, &job)
}

/// Reads the run to its end; the commands go to the progress lines.
fn follow(ctx: &Ctx<'_>, agent: &Found, job: &Job) -> Result<Answer, CliError> {
    let (mut session, mut commands) = (None, 0);
    let mut model = None;
    let mut limits: Option<(Limits, Limits)> = None;
    while let Some(event) = job.next() {
        match event {
            Event::Session(id) => session = Some(id),
            Event::Command(line) => {
                commands += 1;
                if let Some(out) = ctx.local.progress {
                    out(&format!("{} > {line}", agent_name(agent.agent)));
                }
            }
            Event::Done { .. } | Event::Said(_) => {}
            Event::Usage(usage) => model = usage.model.as_deref().map(model_name),
            Event::Limits(now) => limits = Some((limits.map_or(now, |(first, _)| first), now)),
            Event::Finished(Err(why)) => {
                return Err(CliError::Failed(format!(
                    "{}: {why}",
                    agent_name(agent.agent)
                )));
            }
            Event::Finished(Ok(said)) => {
                let more = session.as_ref().map_or(String::new(), |id| {
                    format!("\nпродолжить: devcast beautify --resume {id} --ask \"…\"; отменить всё — devcast undo")
                });
                let spent = spent(model.as_deref(), limits);
                let text = format!("{said}\nкоманд: {commands}{spent}{more}");
                let json = json!({
                    "said": said,
                    "commands": commands,
                    "session": session,
                    "model": model,
                    "limits": limits.map(|(before, after)| json!({
                        "five_hour": [before.five_hour, after.five_hour],
                        "seven_day": [before.seven_day, after.seven_day],
                    })),
                });
                return Ok(Answer::new(text, json));
            }
        }
    }
    Err(CliError::Failed(
        "нейронка закончила работу без ответа: попробуй ещё раз".to_owned(),
    ))
}

/// «, Opus 5.5, лимит 5 ч 19 % → 21 %, неделя 35 %»: the model and how the subscription's limits
/// moved during the run (Claude Code tells them).
fn spent(model: Option<&str>, limits: Option<(Limits, Limits)>) -> String {
    let mut parts: Vec<String> = model.map(str::to_owned).into_iter().collect();
    if let Some((before, after)) = limits {
        let window = |name: &str, b: Option<f64>, a: Option<f64>| {
            let percent = |s: f64| format!("{:.0} %", s * 100.0);
            match (b, a) {
                (Some(b), Some(a)) if percent(b) != percent(a) => {
                    Some(format!("{name} {} → {}", percent(b), percent(a)))
                }
                (_, Some(a)) => Some(format!("{name} {}", percent(a))),
                _ => None,
            }
        };
        parts.extend(window("лимит 5 ч", before.five_hour, after.five_hour));
        parts.extend(window("неделя", before.seven_day, after.seven_day));
    }
    parts.iter().map(|p| format!(", {p}")).collect()
}

/// The recipes named, by id or title; none named — the checked ones.
fn recipes(
    ai: &dc_settings::AiChoices,
    names: &[String],
) -> Result<Vec<dc_settings::Recipe>, CliError> {
    if names.is_empty() {
        return Ok(ai.checked_recipes());
    }
    let all = ai.recipes();
    names
        .iter()
        .map(|name| {
            all.iter()
                .find(|r| r.id == *name || r.title == *name)
                .cloned()
                .ok_or_else(|| {
                    CliError::Usage(format!(
                        "рецепта «{name}» нет: id и названия — devcast recipes ls"
                    ))
                })
        })
        .collect()
}

fn agent_of(name: &str) -> Agent {
    match name {
        "codex" => Agent::Codex,
        _ => Agent::Claude,
    }
}

fn no_agent() -> CliError {
    CliError::Failed(
        "нейронка не найдена: поставь Claude Code (claude.com/claude-code) или Codex CLI и войди в свою подписку; ключи DevCast не хранит".to_owned(),
    )
}

/// An id of this run's undo step, new for every run.
fn group_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("ai-{nanos}")
}

fn failed(error: &dyn std::fmt::Display) -> CliError {
    CliError::Failed(format!(
        "нейронку не удалось запустить ({error}): проверь, что она стоит и запускается"
    ))
}

#[cfg(test)]
#[path = "beautify_tests.rs"]
mod tests;
