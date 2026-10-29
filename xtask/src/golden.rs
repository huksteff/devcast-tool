//! `cargo xtask golden update "<почему>"` (decision 012): rewrites the reference frames by running
//! the `golden` test binaries of the workspace with `UPDATE_GOLDEN=1` and lists what changed. The
//! reason is required: it goes into the commit message (PROCESS.md, «Эталоны»).

use crate::run::{self, Outcome};

const USAGE: &str = "нужно: cargo xtask golden update \"<почему картинка должна была измениться>\"";

pub fn run(args: &[String]) -> Result<String, String> {
    let reason = match args {
        [command, reason @ ..] if command == "update" => reason.join(" "),
        _ => return Err(USAGE.to_owned()),
    };
    if reason.trim().is_empty() {
        return Err(USAGE.to_owned());
    }
    let tests = [
        "nextest",
        "run",
        "--workspace",
        "--cargo-quiet",
        "--no-tests=pass",
        "--hide-progress-bar",
        "--status-level=fail",
        "-E",
        "binary(golden)",
    ];
    let hint = "Эталоны не записались: почини упавший тест и повтори.";
    if let Outcome::Fail(text) = run::cargo_with(&tests, &[("UPDATE_GOLDEN", "1")], hint) {
        return Err(text);
    }
    let status = run::git(&[
        "status",
        "--porcelain",
        "--untracked-files=all",
        "--",
        "fixtures/golden",
    ])?;
    if status.trim().is_empty() {
        return Ok("эталоны не изменились".to_owned());
    }
    Ok(format!(
        "эталоны обновлены:
{}
→ Открой изменённые PNG и проверь глазами. В сообщение коммита: {reason}",
        status.trim_end()
    ))
}
