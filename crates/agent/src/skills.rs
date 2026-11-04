//! The skill for agents the user opens without DevCast (8.10, owner's decision 2026-11-04): a
//! folder with the skill in the forms agents read — `SKILL.md` (Claude Code, Codex skills),
//! `AGENTS.md` (Cursor, Gemini, Codex and the rest) — and `README.md`, which tells the agent how
//! to install it. The user hands the agent the folder and [`ask`]; the agent installs itself.

use std::path::Path;

use crate::prompt::SKILL;

/// The folder of the files in the app settings folder (`%APPDATA%\DevCast`).
pub const FOLDER: &str = "agent-skill";

/// The skill without the front matter of a Claude Code skill.
fn body() -> &'static str {
    let Some(rest) = SKILL.strip_prefix("---") else {
        return SKILL;
    };
    match rest.find("\n---") {
        Some(end) => rest[end + 4..].trim_start(),
        None => SKILL,
    }
}

/// How the agent installs the skill; `devcast` is the folder of `devcast.exe`.
fn readme(devcast: &Path) -> String {
    format!(
        "# Скилл DevCast\n\n\
         Инструкция, как монтировать видео DevCast командами `devcast`. Установи её себе так, \
         как принято у тебя, и дальше монтируй по ней:\n\n\
         - Claude Code: скопируй `SKILL.md` в `~/.claude/skills/devcast/SKILL.md`.\n\
         - Codex: скопируй `SKILL.md` в `~/.codex/skills/devcast/SKILL.md`; если твоя версия не \
         знает skills — допиши `AGENTS.md` в `~/.codex/AGENTS.md`.\n\
         - Gemini CLI: допиши `AGENTS.md` в `~/.gemini/GEMINI.md`.\n\
         - Cursor: создай правило пользователя (User Rules) или `.cursor/rules/devcast.mdc` в \
         проекте с текстом `AGENTS.md`.\n\
         - Другая нейронка: положи `AGENTS.md` туда, откуда ты берёшь постоянные инструкции.\n\n\
         Программа `devcast` лежит в `{}`; если её нет в `PATH`, вызывай её по полному пути.\n\
         Проверка: `devcast help` печатает список команд.\n",
        devcast.display()
    )
}

/// The files of the folder: name and text.
pub fn files(devcast: &Path) -> [(&'static str, String); 3] {
    [
        ("SKILL.md", SKILL.to_owned()),
        ("AGENTS.md", body().to_owned()),
        ("README.md", readme(devcast)),
    ]
}

/// Writes the files into `dir` (created when missing), replacing older ones.
pub fn write(dir: &Path, devcast: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, text) in files(devcast) {
        std::fs::write(dir.join(name), text)?;
    }
    Ok(())
}

/// What the user says to their agent.
pub fn ask(dir: &Path) -> String {
    format!(
        "Установи себе скилл DevCast из папки «{}»: как — написано в README.md. Потом монтируй \
         видео DevCast командами devcast по этому скиллу.",
        dir.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_has_the_skill_for_every_kind_of_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Папка скилла");
        let devcast = Path::new(r"C:\Program Files\DevCast");
        write(&dir, devcast).unwrap();
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
        assert_eq!(read("SKILL.md"), SKILL);
        let agents = read("AGENTS.md");
        assert!(
            agents.starts_with("# Монтаж через devcast"),
            "no front matter"
        );
        assert!(SKILL.ends_with(&agents));
        let readme = read("README.md");
        assert!(readme.contains("~/.claude/skills/devcast/SKILL.md"));
        assert!(readme.contains(r"C:\Program Files\DevCast"));
        assert!(ask(&dir).contains("Папка скилла"));
    }
}
