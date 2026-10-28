//! Commands: the only way to change a [`Project`]. Buttons, hotkeys, the CLI and batch plans all
//! go through a `Command`; [`crate::history::History`] applies them and records undo/redo.

use std::fmt;

use crate::effect::StateError;
use crate::project::Project;

/// Why a command could not be applied. The message says what is wrong and what to do instead.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct CommandError {
    message: String,
}

impl CommandError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<StateError> for CommandError {
    fn from(error: StateError) -> Self {
        Self::new(error.to_string())
    }
}

pub trait Command: fmt::Debug + Send + Sync {
    /// CLI-style name, e.g. `"zoom add"`; shown in history.
    fn name(&self) -> &'static str;

    /// Changes `project`. Returns a one-line summary of what changed, e.g. `"+1 zoom z3"`.
    /// Must leave the project consistent ([`Project::problems`] empty); otherwise the change is
    /// rolled back and reported as a bug in the command.
    fn apply(&self, project: &mut Project) -> Result<String, CommandError>;
}
