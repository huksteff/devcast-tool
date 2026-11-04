//! `devcast recipes`: what «Сделать красиво» asks the AI agent — the recipes, which are
//! checked, the agent and its model. They are app settings (`settings.json` `ai`), edited the
//! way `config` is: by the running app, else in the file (`app_settings::apply`).

use clap::Subcommand;
use dc_agent::{Places, find};
use dc_settings::{Agent, AiChoices, AiEdit, Edit};
use serde_json::{Value, json};

use crate::app_settings::{self, changed_text};
use crate::edit::Ctx;
use crate::error::CliError;
use crate::run::Answer;

#[derive(Debug, Subcommand)]
pub(crate) enum RecipesCommand {
    #[command(
        about = "Рецепты «Сделать красиво» (отмеченные — [x]), найденные нейронки, выбранная нейронка и модель",
        after_help = "recipes ls"
    )]
    Ls,
    #[command(
        about = "Добавить свой рецепт (сразу отмечен) или заменить свой: название и что попросить у нейронки",
        after_help = "recipes add \"Лого в углу\" \"Вставь assets/logo.png в правый нижний угол на всё видео\""
    )]
    Add {
        #[arg(help = "Название в списке")]
        name: String,
        #[arg(help = "Что попросить у нейронки")]
        prompt: String,
        #[arg(
            long,
            help = "Заменить свой рецепт с этим названием (переименование сохраняет отметку)"
        )]
        replace: Option<String>,
    },
    #[command(
        about = "Удалить свой рецепт",
        after_help = "recipes rm \"Лого в углу\""
    )]
    Rm {
        #[arg(help = "Название своего рецепта")]
        name: String,
    },
    #[command(
        about = "Отметить рецепты для «Сделать красиво» (--off — снять отметку)",
        after_help = "recipes check slips keys"
    )]
    Check {
        #[arg(
            required = true,
            help = "id встроенных (pauses, zooms …) или названия своих — devcast recipes ls"
        )]
        ids: Vec<String>,
        #[arg(long, help = "Снять отметку")]
        off: bool,
    },
    #[command(
        about = "Какую нейронку звать, если найдены обе: claude, codex или auto (первую найденную)",
        after_help = "recipes agent codex"
    )]
    Agent {
        #[arg(value_parser = ["claude", "codex", "auto"])]
        agent: String,
    },
    #[command(
        about = "Модель нейронки (sonnet, opus, gpt-5 …) или default — какая у неё по умолчанию",
        after_help = "recipes model sonnet"
    )]
    Model { model: String },
    #[command(
        about = "Войти в подписку нейронки (Claude Code: claude auth login): откроется браузер",
        after_help = "recipes login"
    )]
    Login,
}

pub(crate) fn execute(ctx: &Ctx<'_>, command: RecipesCommand) -> Result<Answer, CliError> {
    let edits = match command {
        RecipesCommand::Ls => return ls(ctx),
        RecipesCommand::Login => return login(ctx),
        RecipesCommand::Add {
            name,
            prompt,
            replace,
        } => vec![AiEdit::Put {
            old: replace,
            name,
            prompt,
        }],
        RecipesCommand::Rm { name } => vec![AiEdit::Remove(name)],
        RecipesCommand::Check { ids, off } => {
            let known = ctx.app_settings()?.ai.recipes();
            for id in &ids {
                if !known.iter().any(|r| r.id == *id) {
                    return Err(CliError::Usage(format!(
                        "рецепта «{id}» нет: id и названия — devcast recipes ls"
                    )));
                }
            }
            ids.into_iter()
                .map(|id| AiEdit::Check { id, on: !off })
                .collect()
        }
        RecipesCommand::Agent { agent } => vec![AiEdit::Agent(match agent.as_str() {
            "claude" => Some(Agent::Claude),
            "codex" => Some(Agent::Codex),
            _ => None,
        })],
        RecipesCommand::Model { model } => {
            vec![AiEdit::Model((model != "default").then_some(model))]
        }
    };
    let before = ctx.app_settings()?.ai;
    let mut by = None;
    let mut after = before.clone();
    for edit in edits {
        let (applied, applied_by) = app_settings::apply(ctx, Edit::Ai(edit))?;
        after = applied.settings.ai;
        by = Some(applied_by);
    }
    let by = by.ok_or_else(|| CliError::Usage("нечего менять".to_owned()))?;
    let lines = changes(&before, &after);
    let json = json!({ "by": by.json(), "ai": choices_json(&after) });
    Ok(Answer::new(changed_text(by, lines), json))
}

fn ls(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let ai = ctx.app_settings()?.ai;
    let found = find(&Places::here());
    let chosen = dc_agent::pick(&found, ai.agent);
    let signed = chosen.and_then(dc_agent::sign_in::signed_in);
    let mut lines = vec![format!("нейронка: {}", agents_line(&ai, &found))];
    if signed == Some(false) {
        lines.push("не вошла в подписку: devcast recipes login".to_owned());
    }
    lines.push(format!(
        "модель: {}",
        ai.model.as_deref().unwrap_or("по умолчанию")
    ));
    for recipe in ai.recipes() {
        let mark = if ai.checked.contains(&recipe.id) {
            "[x]"
        } else {
            "[ ]"
        };
        let own = if recipe.own { " (свой)" } else { "" };
        lines.push(format!("{mark} {} — {}{own}", recipe.id, recipe.title));
    }
    let found: Vec<Value> = found
        .iter()
        .map(|f| json!({ "agent": f.agent, "path": f.path }))
        .collect();
    let json = json!({ "found": found, "signed_in": signed, "ai": choices_json(&ai) });
    Ok(Answer::new(lines.join("\n"), json))
}

/// `recipes login`: the agent's own sign-in, in this console (a browser page opens).
fn login(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let ai = ctx.app_settings()?.ai;
    let found = find(&Places::here());
    let agent = dc_agent::pick(&found, ai.agent).ok_or_else(|| {
        CliError::Failed("нейронка не найдена: поставь Claude Code или Codex".to_owned())
    })?;
    let name = agent_name(agent.agent);
    if ctx.globals.dry_run {
        let text = format!("dry-run: {name} не запущен");
        return Ok(Answer::new(text, json!({ "dry_run": true })));
    }
    let status = std::process::Command::new(&agent.path)
        .args(dc_agent::sign_in::login_args(agent.agent))
        .status()
        .map_err(|e| {
            CliError::Failed(format!("{name} не запустился ({e}): проверь, что он стоит"))
        })?;
    let signed = dc_agent::sign_in::signed_in(agent);
    if !status.success() || signed == Some(false) {
        return Err(CliError::Failed(format!(
            "{name} не вошёл в подписку: запусти devcast recipes login ещё раз"
        )));
    }
    Ok(Answer::new(
        format!("{name}: вход выполнен"),
        json!({ "signed_in": signed }),
    ))
}

fn agents_line(ai: &AiChoices, found: &[dc_agent::Found]) -> String {
    let names: Vec<&str> = found.iter().map(|f| agent_name(f.agent)).collect();
    let chosen = dc_agent::pick(found, ai.agent).map(|f| agent_name(f.agent));
    match (chosen, names.len()) {
        (None, _) => "не найдена — нужен Claude Code или Codex, вошедший в подписку".to_owned(),
        (Some(name), 1) => name.to_owned(),
        (Some(name), _) => format!("{name} (найдены: {})", names.join(", ")),
    }
}

pub(crate) fn agent_name(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "Claude Code",
        Agent::Codex => "Codex",
    }
}

fn choices_json(ai: &AiChoices) -> Value {
    let recipes: Vec<Value> = ai
        .recipes()
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "title": r.title,
                "prompt": r.prompt,
                "own": r.own,
                "checked": ai.checked.contains(&r.id),
            })
        })
        .collect();
    json!({ "agent": ai.agent, "model": ai.model, "recipes": recipes })
}

/// What changed, a line each.
fn changes(before: &AiChoices, after: &AiChoices) -> Vec<String> {
    let mut lines = Vec::new();
    for recipe in after.recipes() {
        let old = before.recipes().into_iter().find(|r| r.id == recipe.id);
        match old {
            None => lines.push(format!("рецепт «{}» добавлен", recipe.title)),
            Some(old) if old.prompt != recipe.prompt => {
                lines.push(format!("рецепт «{}» изменён", recipe.title));
            }
            _ => {}
        }
        let (was, is) = (
            before.checked.contains(&recipe.id),
            after.checked.contains(&recipe.id),
        );
        if was != is {
            let state = if is {
                "отмечен"
            } else {
                "не отмечен"
            };
            lines.push(format!("{}: {state}", recipe.id));
        }
    }
    for old in before.recipes() {
        if !after.recipes().iter().any(|r| r.id == old.id) {
            lines.push(format!("рецепт «{}» удалён", old.title));
        }
    }
    if before.agent != after.agent {
        let name = after.agent.map_or("auto", agent_name);
        lines.push(format!("нейронка: {name}"));
    }
    if before.model != after.model {
        let name = after.model.as_deref().unwrap_or("по умолчанию");
        lines.push(format!("модель: {name}"));
    }
    lines
}
