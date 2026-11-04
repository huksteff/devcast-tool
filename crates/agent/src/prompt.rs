//! What the agent is told: the system part — the brief `brief.md`, the editing part of the skill
//! `skill/SKILL.md` with the rules of a run, short because the agent reads it again on every
//! step (owner's decision 2026-11-04: as few tokens as the work allows) — and the request — the
//! checked recipes and the user's own words. Nothing needs to be installed.

use dc_settings::Recipe;

/// The skill of `devcast`, the same file `devcast skill install` puts for Claude Code.
pub const SKILL: &str = include_str!("../../../skill/SKILL.md");

/// The brief of a run: how to edit a project with `devcast` and the rules of «Сделать красиво».
pub const BRIEF: &str = include_str!("brief.md");

/// The system part of every run.
pub fn system() -> String {
    BRIEF.to_owned()
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
    fn the_system_part_is_the_short_brief() {
        let system = system();
        assert!(system.contains("только командами `devcast`"));
        assert!(
            system.len() * 4 < SKILL.len(),
            "a quarter of the skill at most"
        );
    }
}
