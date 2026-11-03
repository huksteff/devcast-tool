//! «Сделать красиво» (8.11): the user's AI agent — Claude Code or Codex, whichever is on the
//! computer, on the user's own subscription — edits the project with `devcast` commands. The
//! skill `skill/SKILL.md` goes with the request, recipes are ready requests. Shared by the
//! editor's «Нейронка» tab and `devcast beautify`.

pub mod find;
pub mod prompt;
pub mod run;
pub mod stream;

pub use find::{Found, Places, find, pick};
pub use run::{Ask, GROUP_ENV, Job, Launch, launch};
pub use stream::{Event, Reader};

/// Writes [`prompt::system`] where the agent reads it (`--append-system-prompt-file`): the
/// temp folder of the user.
pub fn system_file() -> std::io::Result<std::path::PathBuf> {
    let path = std::env::temp_dir().join("devcast-agent-skill.md");
    std::fs::write(&path, prompt::system())?;
    Ok(path)
}
