//! What the agent is told: the system part — the skill `skill/SKILL.md` and the rules of the
//! editor — and the request — the checked recipes and the user's own words. The skill goes with
//! every run, so nothing needs to be installed.

use dc_settings::Recipe;

/// The skill of `devcast`, the same file `devcast skill install` puts for Claude Code.
pub const SKILL: &str = include_str!("../../../skill/SKILL.md");

/// The rules of a run inside the editor, after the skill.
const RULES: &str = "\
## Тебя запустило «Сделать красиво»

Текущая папка — проект `*.devcast`. Если он открыт в редакторе DevCast, каждая команда \
`devcast` сразу видна пользователю. Всё, что ты сделаешь за этот запуск, отменяется одним шагом \
(Ctrl+Z в редакторе, `devcast undo` в консоли).

- Меняй и смотри проект только командами `devcast`. Файлы проекта не читай и не правь руками.
- Не экспортируй, не сохраняй (`devcast save`), не закрывай и не переименовывай проект и не \
трогай программу (`record`, `app`, `config`, `hotkeys`, `recipes`, `beautify`): это решает \
пользователь.
- Перед вырезами и пачками — `--dry-run`, как сказано выше.
- Команда отказала — прочитай ошибку и исправь вызов; не получается — пропусти этот шаг.
- В конце одним-двумя предложениями по-русски напиши, что сделал (без списка команд).";

/// The system part of every run.
pub fn system() -> String {
    format!("{SKILL}\n\n{RULES}\n")
}

/// The first request of a conversation: the recipes in order, then `ask`. `None` when there is
/// nothing to ask for.
pub fn first(recipes: &[Recipe], ask: &str) -> Option<String> {
    let ask = ask.trim();
    if recipes.is_empty() && ask.is_empty() {
        return None;
    }
    let mut text = String::from("Сделай монтаж этого проекта.");
    if !recipes.is_empty() {
        text.push_str(" По порядку:\n");
        for (n, recipe) in recipes.iter().enumerate() {
            text.push_str(&format!("{}. {}\n", n + 1, recipe.prompt));
        }
    }
    if !ask.is_empty() {
        text.push_str(&format!("\nЕщё пользователь просит:\n{ask}\n"));
    }
    Some(text)
}

/// A request that goes on with the conversation («Доделать или исправить»).
pub fn more(ask: &str) -> Option<String> {
    let ask = ask.trim();
    (!ask.is_empty()).then(|| format!("Пользователь посмотрел результат и просит:\n{ask}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(prompt: &str) -> Recipe {
        Recipe {
            id: prompt.to_owned(),
            title: prompt.to_owned(),
            prompt: prompt.to_owned(),
            own: false,
        }
    }

    #[test]
    fn the_request_is_the_recipes_in_order_then_the_users_words() {
        let text = first(
            &[recipe("Убери паузы."), recipe("Сделай субтитры.")],
            " не трогай 0:42 ",
        );
        let text = text.unwrap();
        let pauses = text.find("1. Убери паузы.").unwrap();
        let captions = text.find("2. Сделай субтитры.").unwrap();
        let ask = text.find("не трогай 0:42").unwrap();
        assert!(pauses < captions && captions < ask);
        assert_eq!(first(&[], "  "), None, "nothing to do");
        assert!(first(&[], "только зумы").unwrap().contains("только зумы"));
        assert_eq!(more(""), None);
    }

    #[test]
    fn the_system_part_is_the_skill_and_the_rules() {
        let system = system();
        assert!(system.starts_with(SKILL));
        assert!(system.contains("только командами `devcast`"));
    }
}
