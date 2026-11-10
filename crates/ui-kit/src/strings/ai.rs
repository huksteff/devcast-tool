//! The «Нейронка» tab of the editor (mockup 9): «Сделать красиво» by the user's AI agent.

pub const TAB: &str = "Нейронка";
pub const TITLE: &str = "Сделать красиво";
pub const SUBTITLE: &str = "нейронка правит проект командами devcast";
pub const RECIPES: &str = "Рецепты";
pub const OWN_RECIPE: &str = "Свой рецепт";
pub const EDIT_RECIPE: &str = "Изменить рецепт";
pub const DELETE_RECIPE: &str = "Удалить рецепт";
pub const ASK: &str = "Ещё попросить";
pub const ASK_HINT: &str = "например: «не режь паузу на 0:42»";
pub const AGENT: &str = "Нейронка";
pub const MODEL: &str = "Модель";
pub const MODEL_DEFAULT: &str = "По умолчанию";
pub const OPUS: &str = "Opus — сильнее, тратит больше";
pub const SONNET: &str = "Sonnet — быстрее и экономнее";
pub const HAIKU: &str = "Haiku — самый быстрый";
pub const RUN: &str = "Сделать красиво";
pub const NOTHING: &str = "Отметь рецепт или напиши, что попросить.";
pub const LOG: &str = "Ход работы";
pub const STOP: &str = "Остановить";
pub const WHILE_RUNNING: &str = "Пока нейронка работает, проект можно править: её команда, попавшая на вашу правку, повторится.";
pub const UNDO_ALL: &str = "Отменить всё (Ctrl+Z)";
pub const MORE: &str = "Доделать или исправить";
pub const MORE_HINT: &str = "например: «верни паузу на 0:42»";
pub const SEND: &str = "Отправить";
pub const NEW_RUN: &str = "Начать заново";
pub const NOT_FOUND: &str = "нейронка не найдена";
pub const NOT_FOUND_TEXT: &str =
    "Нужен Claude Code или Codex, вошедший в вашу подписку. Ключи DevCast не хранит.";
pub const HOW_TO_INSTALL: &str = "Как поставить";
/// Where «Как поставить» leads: how to set up Claude Code.
pub const INSTALL_URL: &str = "https://docs.anthropic.com/en/docs/claude-code/setup";
pub const CHECK_AGAIN: &str = "Проверить снова";
pub const SIGNED_OUT: &str = "нейронка не вошла в аккаунт";
pub const SIGNED_OUT_TEXT: &str = "Claude Code не вошёл в вашу подписку. «Войти» откроет чёрное окно и страницу в браузере. Разрешите доступ; если покажут код — вставьте его в чёрное окно и нажмите Enter. Потом «Проверить снова».";
pub const SIGN_IN: &str = "Войти";
pub const OWN_AGENT: &str = "Своя нейронка";
pub const OWN_AGENT_TEXT: &str = "Чтобы монтировать нейронкой вне DevCast (Claude Code, Codex, Cursor, Gemini …), отдайте ей эту папку и попросите установить скилл:";
pub const COPY_ASK: &str = "Копировать просьбу";
pub const OPEN_FOLDER: &str = "Открыть папку";
pub const RECIPE_NAME: &str = "Название";
pub const RECIPE_NAME_HINT: &str = "например: «Лого в углу»";
pub const RECIPE_PROMPT: &str = "Что попросить у нейронки";
pub const SAVE: &str = "Сохранить";
pub const CANCEL: &str = "Отмена";
pub const CLAUDE: &str = "Claude Code";
pub const CODEX: &str = "Codex";

/// «работает · 0:48»: `secs` since the start.
pub fn running(secs: u64) -> String {
    format!("работает · {}", clock(secs))
}

/// «готово · 1:32 · 9 команд».
pub fn done(secs: u64, commands: usize) -> String {
    format!("готово · {} · {}", clock(secs), commands_count(commands))
}

/// «остановлено · 0:20 · 3 команды».
pub fn stopped(secs: u64, commands: usize) -> String {
    format!(
        "остановлено · {} · {}",
        clock(secs),
        commands_count(commands)
    )
}

/// «не получилось · 0:05».
pub fn failed(secs: u64) -> String {
    format!("не получилось · {}", clock(secs))
}

/// «9 команд» with the Russian plural.
pub fn commands_count(n: usize) -> String {
    let word = match (n % 10, n % 100) {
        (1, 11) | (2..=4, 12..=14) | (0 | 5..=9, _) => "команд",
        (1, _) => "команда",
        _ => "команды",
    };
    format!("{n} {word}")
}

/// The agent could not be started, and why.
pub fn not_started(error: &dyn std::fmt::Display) -> String {
    format!("Нейронку не удалось запустить ({error}): проверь, что она стоит и запускается.")
}

fn clock(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// The skill folder for agents opened without DevCast could not be written.
pub fn skill_not_written(error: &dyn std::fmt::Display) -> String {
    format!(
        "Скилл для своей нейронки не записан ({error}): проверьте место на диске и права на папку настроек."
    )
}

/// «Лимит 5 ч: 19 % → 21 % · неделя: 35 % → 36 %»: how full the subscription's limits were
/// before and after a run, shares 0..1; one number when they did not change.
pub fn limits(five_hour: (Option<f64>, Option<f64>), week: (Option<f64>, Option<f64>)) -> String {
    let part = |name: &str, (before, after): (Option<f64>, Option<f64>)| {
        let percent = |share: f64| format!("{:.0} %", share * 100.0);
        match (before, after) {
            (Some(b), Some(a)) if percent(b) != percent(a) => {
                Some(format!("{name}: {} → {}", percent(b), percent(a)))
            }
            (_, Some(a)) => Some(format!("{name}: {}", percent(a))),
            _ => None,
        }
    };
    let parts: Vec<String> = [part("лимит 5 ч", five_hour), part("неделя", week)]
        .into_iter()
        .flatten()
        .collect();
    let line = parts.join(" · ");
    let mut chars = line.chars();
    chars
        .next()
        .map_or(line.clone(), |c| c.to_uppercase().chain(chars).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_times_read_as_russian() {
        let counts: Vec<String> = [1, 2, 5, 11, 12, 21, 23, 0].map(commands_count).into();
        let expected = [
            "1 команда",
            "2 команды",
            "5 команд",
            "11 команд",
            "12 команд",
            "21 команда",
            "23 команды",
            "0 команд",
        ];
        assert_eq!(counts, expected);
        assert_eq!(done(92, 9), "готово · 1:32 · 9 команд");
        assert_eq!(running(48), "работает · 0:48");
        let both = limits((Some(0.19), Some(0.21)), (Some(0.35), Some(0.35)));
        assert_eq!(both, "Лимит 5 ч: 19 % → 21 % · неделя: 35 %");
    }
}
