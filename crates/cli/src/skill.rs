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
