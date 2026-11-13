//! Whether the agent is signed in to the user's account, and signing in: Claude Code answers
//! `claude auth status` as JSON (`loggedIn`) and signs in with `claude auth login`, Codex with
//! `codex login status` / `codex login`. The app's piped flow lives in `login`; the CLI uses
//! the agent's interactive login command.

use std::process::{Command, Stdio};

use dc_settings::Agent;

use crate::find::Found;

/// Signed in; none when the agent cannot tell.
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
        Agent::Codex => {
            let mut status = Command::new(&found.path);
            status.args(["login", "status"]).stdin(Stdio::null());
            crate::run::no_window(&mut status);
            let output = status.output().ok()?;
            let all = [output.stdout, output.stderr].concat();
            codex_status(output.status.success(), &String::from_utf8_lossy(&all))
        }
    }
}

/// Whether the agent really works on the user's subscription: asks it for one word (a few
/// seconds and tokens). The check after a run was [`refused`]: the status goes on telling
/// «signed in» with a token that is no longer taken. None when the agent cannot be asked.
pub fn answers(found: &Found) -> Option<bool> {
    match found.agent {
        Agent::Claude => {
            let mut ask = Command::new(&found.path);
            ask.args(["-p", "ok", "--model", "haiku"])
                .stdin(Stdio::null());
            crate::run::no_window(&mut ask);
            let said = ask.output().ok()?;
            let all = [said.stdout, said.stderr].concat();
            Some(said.status.success() && !refused(&String::from_utf8_lossy(&all)))
        }
        Agent::Codex => signed_in(found),
    }
}

/// Whether `why`, the reason a run failed, says the agent is not signed in: its status may
/// still tell «signed in» while the token it keeps is refused (expired, revoked).
pub fn refused(why: &str) -> bool {
    let why = why.to_lowercase();
    [
        "401",
        "failed to authenticate",
        "session has expired",
        "not logged in",
        "/login",
        "codex login",
        "refresh token has already been used",
        "refresh_token_reused",
    ]
    .iter()
    .any(|sign| why.contains(sign))
}

// Codex writes status to stderr; an unsupported command or broken config is unknown,
// rather than a reason to ask the user to sign in again.
fn codex_status(success: bool, status: &str) -> Option<bool> {
    let status = status.to_lowercase();
    if status.contains("not logged in") {
        Some(false)
    } else if success && status.contains("logged in") {
        Some(true)
    } else {
        None
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
    fn codex_reports_login_without_reading_credentials() {
        assert_eq!(codex_status(true, "Logged in using ChatGPT"), Some(true));
        assert_eq!(codex_status(true, "Logged in using an API key"), Some(true));
        assert_eq!(codex_status(false, "Not logged in"), Some(false));
        assert_eq!(codex_status(false, "error: invalid configuration"), None);
        assert_eq!(codex_status(false, "error: unknown command 'status'"), None);
        assert!(refused("Please run codex login"));
        assert!(refused("refresh_token_reused"));
        assert_eq!(login_args(Agent::Codex), ["login"]);
    }

    #[cfg(windows)]
    #[test]
    fn codex_status_is_read_from_its_stderr_and_exit_code() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("codex.cmd");
        let found = Found {
            agent: Agent::Codex,
            path: path.clone(),
        };
        for (message, code, expected) in [
            ("Logged in using ChatGPT", 0, Some(true)),
            ("Not logged in", 1, Some(false)),
            ("error: invalid configuration", 1, None),
        ] {
            std::fs::write(&path, format!("@echo off\r\nif not \"%1 %2\"==\"login status\" exit /b 2\r\necho {message} 1>&2\r\nexit /b {code}\r\n")).unwrap();
            assert_eq!(signed_in(&found), expected, "{message}");
        }
    }

    #[test]
    fn the_status_says_whether_claude_code_is_signed_in() {
        // As Claude Code 2.1.309 prints it.
        let out = "{\n  \"loggedIn\": false,\n  \"authMethod\": \"none\",\n  \"apiProvider\": \"firstParty\"\n}\n";
        assert_eq!(logged_in(out), Some(false));
        assert_eq!(logged_in(r#"{"loggedIn": true}"#), Some(true));
        assert_eq!(logged_in("error: unknown command"), None);
        assert_eq!(login_args(Agent::Claude), ["auth", "login"]);
        // As Claude Code 2.1.321 fails with a token that went stale.
        assert!(refused(
            "Failed to authenticate. API Error: 401 OAuth access token is invalid."
        ));
        assert!(refused(
            "Your Claude session has expired. Sign in again to continue (claude auth login)."
        ));
        assert!(!refused(
            "нейронка закончила работу без ответа: попробуй ещё раз"
        ));
    }
}
