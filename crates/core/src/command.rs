//! Commands: the only way to change a [`Project`]. Buttons, hotkeys, the CLI and batch plans all
//! go through a `Command`; [`crate::history::History`] applies them and records undo/redo.

use std::fmt;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;

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

/// A command the CLI and plans can name. Its fields are its arguments, so one type serves the
/// command line (`zoom add --at 12.5`), plans (`{"op":"zoom add","at":12.5}`) and `help --json`.
/// Declare `#[serde(deny_unknown_fields)]`, so a mistyped argument is an error, not ignored.
pub trait Op: Command + DeserializeOwned + JsonSchema + 'static {
    /// Same as [`Command::name`].
    const NAME: &'static str;
    /// One line in Russian for `devcast help`.
    const ABOUT: &'static str;
    /// A working command line without `devcast`, e.g. `"zoom add --at 12.5 --scale 1.8"`.
    const EXAMPLE: &'static str;
    /// Arguments given without `--name`, in order.
    const POSITIONAL: &'static [&'static str] = &[];
}

/// Everything about an [`Op`] that the ops registry needs, without its type.
#[derive(Clone)]
pub struct OpInfo {
    pub name: &'static str,
    pub about: &'static str,
    pub example: &'static str,
    pub positional: &'static [&'static str],
    pub schema: fn() -> schemars::Schema,
    pub parse: fn(serde_json::Value) -> Result<Box<dyn Command>, String>,
}

impl OpInfo {
    pub fn of<T: Op>() -> Self {
        Self {
            name: T::NAME,
            about: T::ABOUT,
            example: T::EXAMPLE,
            positional: T::POSITIONAL,
            schema: || schemars::schema_for!(T),
            parse: |args| {
                serde_json::from_value::<T>(args)
                    .map(|op| Box::new(op) as Box<dyn Command>)
                    .map_err(|e| e.to_string())
            },
        }
    }
}

impl fmt::Debug for OpInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpInfo").field("name", &self.name).finish()
    }
}
