//! `cargo xtask hook-stop`: the Claude Code Stop hook (configured in .claude/settings.json).
//!
//! If code changed and `verify` fails, the hook blocks the stop (exit code 2) and hands the
//! report to the agent, so a session cannot end with "done" on red checks. To avoid an endless
//! loop it blocks only once in a row: when the agent is already continuing because of this hook
//! (`stop_hook_active`), a still-red report is shown to the owner instead.

use std::io::Read;
use std::process::ExitCode;

use crate::{changed, verify};

/// Exit code that tells Claude Code to block the stop and show stderr to the agent.
const BLOCK: u8 = 2;

pub fn stop() -> ExitCode {
    let mut input = String::new();
    // Unreadable input is treated as a first stop attempt: being strict is the safe side.
    let active = std::io::stdin().read_to_string(&mut input).is_ok() && is_active(&input);
    let paths = match changed::uncommitted_paths() {
        Ok(paths) => paths,
        Err(error) => {
            return block(&format!(
                "hook-stop: не удалось получить git status: {error}"
            ));
        }
    };
    if !changed::touches_code(&paths) {
        return ExitCode::SUCCESS;
    }
    let report = verify::run(&verify::Options {
        full: false,
        include_ignored: false,
    });
    match decide(report.is_ok(), active) {
        Decision::Allow => ExitCode::SUCCESS,
        Decision::Block => block(&format!(
            "{}\n\nЗадача не готова: verify красный. Исправь и повтори (docs/PROCESS.md). \
             Если чисто исправить нельзя — остановись и спроси владельца.",
            report.render()
        )),
        Decision::WarnOwner => {
            let message = format!(
                "verify всё ещё красный, сессия остановлена без «готово»:\n{}",
                report.render()
            );
            println!("{}", serde_json::json!({ "systemMessage": message }));
            ExitCode::SUCCESS
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Decision {
    Allow,
    Block,
    WarnOwner,
}

fn decide(verify_ok: bool, active: bool) -> Decision {
    match (verify_ok, active) {
        (true, _) => Decision::Allow,
        (false, false) => Decision::Block,
        (false, true) => Decision::WarnOwner,
    }
}

fn is_active(input: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(input)
        .ok()
        .and_then(|value| value.get("stop_hook_active")?.as_bool())
        .unwrap_or(false)
}

fn block(message: &str) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(BLOCK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn green_verify_always_allows_stop() {
        assert_eq!(decide(true, false), Decision::Allow);
        assert_eq!(decide(true, true), Decision::Allow);
    }

    #[test]
    fn red_verify_blocks_once_then_warns_owner() {
        assert_eq!(decide(false, false), Decision::Block);
        assert_eq!(decide(false, true), Decision::WarnOwner);
    }

    #[test]
    fn reads_stop_hook_active_flag() {
        assert!(is_active(
            r#"{"session_id": "x", "stop_hook_active": true}"#
        ));
        assert!(!is_active(r#"{"stop_hook_active": false}"#));
        assert!(!is_active("not json"));
    }
}
