//! «Сделать красиво» (8.11) remembered between runs: the user's AI agent and its model, the
//! recipes checked and the user's own recipes. The built-in recipes are data of the list, kept
//! here with the own ones so one place checks names (`devcast recipes`, the «Нейронка» tab).

use serde::{Deserialize, Serialize};

/// The AI agent of the user that edits the project with `devcast` commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Agent {
    /// Claude Code, `claude -p`.
    Claude,
    /// Codex CLI, `codex exec`.
    Codex,
}

/// A recipe the user wrote: its name on the list and what to ask the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnRecipe {
    pub name: String,
    pub prompt: String,
}

/// The choices of the «Нейронка» tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiChoices {
    /// The agent chosen when both are found; none — the first one found.
    pub agent: Option<Agent>,
    /// The model the agent is asked for (`sonnet`, `gpt-5`); none — the agent's own default.
    pub model: Option<String>,
    /// Checked recipes: ids of the built-in ones, names of the own ones.
    pub checked: Vec<String>,
    pub own: Vec<OwnRecipe>,
}

/// The built-in recipes checked at first: pauses, zooms by clicks.
pub const CHECKED_FIRST: [&str; 2] = ["pauses", "zooms"];

impl Default for AiChoices {
    fn default() -> Self {
        Self {
            agent: None,
            model: None,
            checked: CHECKED_FIRST.iter().map(|&id| id.to_owned()).collect(),
            own: Vec::new(),
        }
    }
}

/// The built-in recipes: id, title, request to the agent.
pub const BUILT_IN: [(&str, &str, &str); 3] = [
    (
        "pauses",
        "Убрать паузы",
        "Убери паузы в речи: найди их `devcast silences` и вырежи одной командой `devcast cut`, оставляя по 0,15 с речи с краёв каждой паузы.",
    ),
    (
        "zooms",
        "Зумы по кликам",
        "Поставь зумы по кликам: `devcast zoom add --at clicks`. Если где-то зумы идут чаще одного на 4 секунды, убери лишние.",
    ),
    (
        "slips",
        "Убрать оговорки и «ээ»",
        "По `devcast transcript` найди оговорки, повторы, фальстарты и слова-паразиты («ээ», «ну», «как бы») и вырежи их по словам (`devcast cut w…`). Смысл сказанного не меняй.",
    ),
];

/// A recipe on the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    /// Built-in: a fixed id (`pauses`); own: its name, unique on the list.
    pub id: String,
    pub title: String,
    pub prompt: String,
    /// The user's own: it can be edited and removed.
    pub own: bool,
}

/// Why a recipe edit is refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum RecipeError {
    #[error("у рецепта нет названия: напиши, как он будет называться в списке")]
    NoName,
    #[error("рецепт пустой: напиши, что попросить у нейронки")]
    NoPrompt,
    #[error("рецепт «{0}» уже есть: выбери другое название")]
    Taken(String),
    #[error("своего рецепта «{0}» нет: список — devcast recipes ls")]
    Missing(String),
}

/// An edit of the «Нейронка» choices (`devcast recipes`, the tab).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiEdit {
    /// Adds the own recipe `name`, checked, or replaces the own recipe `old` with it (a rename
    /// keeps its check).
    Put {
        old: Option<String>,
        name: String,
        prompt: String,
    },
    /// Removes an own recipe and its check.
    Remove(String),
    /// Checks or unchecks a recipe by its id.
    Check {
        id: String,
        on: bool,
    },
    Agent(Option<Agent>),
    Model(Option<String>),
}

impl AiChoices {
    /// The list of the tab: the built-in recipes, then the user's own.
    pub fn recipes(&self) -> Vec<Recipe> {
        let built_in = BUILT_IN.iter().map(|&(id, title, prompt)| Recipe {
            id: id.to_owned(),
            title: title.to_owned(),
            prompt: prompt.to_owned(),
            own: false,
        });
        let own = self.own.iter().map(|OwnRecipe { name, prompt }| Recipe {
            id: name.clone(),
            title: name.clone(),
            prompt: prompt.clone(),
            own: true,
        });
        built_in.chain(own).collect()
    }

    /// The recipes checked, in the order of the list.
    pub fn checked_recipes(&self) -> Vec<Recipe> {
        self.recipes()
            .into_iter()
            .filter(|r| self.checked.contains(&r.id))
            .collect()
    }

    /// Applies `edit`; refused — nothing changes.
    pub fn edit(&mut self, edit: AiEdit) -> Result<(), RecipeError> {
        match edit {
            AiEdit::Put { old, name, prompt } => self.put(old.as_deref(), &name, &prompt)?,
            AiEdit::Remove(name) => {
                let before = self.own.len();
                self.own.retain(|r| r.name != name);
                if self.own.len() == before {
                    return Err(RecipeError::Missing(name));
                }
                self.checked.retain(|id| *id != name);
            }
            AiEdit::Check { id, on } => {
                self.checked.retain(|c| *c != id);
                if on && self.recipes().iter().any(|r| r.id == id) {
                    self.checked.push(id);
                }
            }
            AiEdit::Agent(agent) => {
                if self.agent != agent {
                    self.model = None;
                }
                self.agent = agent;
            }
            AiEdit::Model(model) => {
                self.model = model.map(|m| m.trim().to_owned()).filter(|m| !m.is_empty());
            }
        }
        Ok(())
    }

    fn put(&mut self, old: Option<&str>, name: &str, prompt: &str) -> Result<(), RecipeError> {
        let (name, prompt) = (name.trim(), prompt.trim());
        if name.is_empty() {
            return Err(RecipeError::NoName);
        }
        if prompt.is_empty() {
            return Err(RecipeError::NoPrompt);
        }
        let taken = self
            .recipes()
            .iter()
            .any(|r| (r.id == name || r.title == name) && Some(r.id.as_str()) != old);
        if taken {
            return Err(RecipeError::Taken(name.to_owned()));
        }
        let recipe = OwnRecipe {
            name: name.to_owned(),
            prompt: prompt.to_owned(),
        };
        let Some(old) = old else {
            self.own.push(recipe);
            self.checked.push(name.to_owned());
            return Ok(());
        };
        let place = self.own.iter().position(|r| r.name == old);
        let place = place.ok_or_else(|| RecipeError::Missing(old.to_owned()))?;
        self.own[place] = recipe;
        for id in &mut self.checked {
            if id == old {
                name.clone_into(id);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "ai_tests.rs"]
mod tests;
