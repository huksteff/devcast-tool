//! Whether the agent is signed in to the user's account, and signing in: Claude Code answers
//! `claude auth status` as JSON (`loggedIn`) and signs in with `claude auth login`, Codex with
//! `codex login`. Signing in needs a person (a browser page), so it opens its own console window.

use std::process::{Command, Stdio};

use dc_settings::Agent;

use crate::find::Found;

/// Signed in; none when the agent cannot tell (Codex has no status to read).
pub fn signed_in(found: &Found) -> Option<bool> {
    match found.agent {
        Agent::Claude => {
            let mut status = Command::new(&found.path);
            status
                .args(["auth", "status"])
                .stdin(Stdio::null())
                .stderr(Stdio::null());
            crate::run::no_window(&mut status);
            let output = status.output().ok()?;
            logged_in(&String::from_utf8_lossy(&output.stdout))
        }
        Agent::Codex => None,
    }
}

/// `loggedIn` of the status JSON.
fn logged_in(status: &str) -> Option<bool> {
    let json: serde_json::Value = serde_json::from_str(status.trim()).ok()?;
    json["loggedIn"].as_bool()
}

/// The sign-in command of the agent.
pub fn login_args(agent: Agent) -> &'static [&'static str] {
    match agent {
        Agent::Claude => &["auth", "login"],
        Agent::Codex => &["login"],
    }
}

/// Opens a console window where the agent signs in; returns once it is open.
pub fn open_login(found: &Found) -> std::io::Result<()> {
    let mut login = Command::new(&found.path);
    login.args(login_args(found.agent));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        login.creation_flags(CREATE_NEW_CONSOLE);
    }
    login.spawn().map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_says_whether_claude_code_is_signed_in() {
        // As Claude Code 2.1.309 prints it.
        let out = "{\n  \"loggedIn\": false,\n  \"authMethod\": \"none\",\n  \"apiProvider\": \"firstParty\"\n}\n";
        assert_eq!(logged_in(out), Some(false));
        assert_eq!(logged_in(r#"{"loggedIn": true}"#), Some(true));
        assert_eq!(logged_in("error: unknown command"), None);
        assert_eq!(login_args(Agent::Claude), ["auth", "login"]);
    }
}
