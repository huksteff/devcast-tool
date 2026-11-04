//! `devcast skill install`: puts the Claude Code skill (`skill/SKILL.md`, built into the binary)
//! into the user's skills folder, replacing an older copy.

use std::path::{Path, PathBuf};

use serde_json::json;

use crate::error::CliError;
use crate::run::Answer;

pub(crate) const SKILL: &str = dc_agent::prompt::SKILL;

/// `~/.claude/skills/devcast/SKILL.md`, where Claude Code looks for personal skills.
pub(crate) fn target() -> Result<PathBuf, CliError> {
    let home = std::env::home_dir().ok_or_else(|| {
        CliError::Failed(
            "домашняя папка не найдена: задай переменную USERPROFILE (Windows) или HOME".to_owned(),
        )
    })?;
    Ok(home
        .join(".claude")
        .join("skills")
        .join("devcast")
        .join("SKILL.md"))
}

pub(crate) fn install(target: &Path, dry_run: bool) -> Result<Answer, CliError> {
    let json = json!({"path": target, "dry_run": dry_run});
    if dry_run {
        let text = format!(
            "dry-run, ничего не записано: skill ляжет в {}",
            target.display()
        );
        return Ok(Answer::new(text, json));
    }
    if let Some(folder) = target.parent() {
        std::fs::create_dir_all(folder).map_err(|e| {
            CliError::Failed(format!(
                "не удалось создать папку «{}» ({e}): проверь права",
                folder.display()
            ))
        })?;
    }
    dc_project::atomic::write(target, SKILL.as_bytes())?;
    Ok(Answer::new(
        format!("skill установлен: {}", target.display()),
        json,
    ))
}

/// `devcast skill files`: the skill for any agent in `agent-skill` of the app settings folder
/// (`dc_agent::skills`), and the request to hand the agent.
pub(crate) fn files(ctx: &crate::edit::Ctx<'_>) -> Result<Answer, CliError> {
    let folder = ctx.settings_dir()?.join(dc_agent::skills::FOLDER);
    let ask = dc_agent::skills::ask(&folder);
    let json = json!({"path": folder, "ask": ask, "dry_run": ctx.globals.dry_run});
    if ctx.globals.dry_run {
        let text = format!(
            "dry-run, ничего не записано: скилл ляжет в {}",
            folder.display()
        );
        return Ok(Answer::new(text, json));
    }
    let exe = std::env::current_exe().map_err(|e| {
        CliError::Failed(format!(
            "не найден путь к devcast ({e}): запусти devcast из его папки"
        ))
    })?;
    let devcast = exe.parent().unwrap_or(&exe);
    dc_agent::skills::write(&folder, devcast).map_err(|e| {
        CliError::Failed(format!(
            "скилл не записан в «{}» ({e}): проверь место на диске и права",
            folder.display()
        ))
    })?;
    let text = format!(
        "скилл для любой нейронки: {}\nскажи нейронке: {ask}",
        folder.display()
    );
    Ok(Answer::new(text, json))
}
