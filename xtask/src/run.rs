//! Running external tools (cargo, git) and turning their result into a check outcome.

use std::process::{Command, Output};

/// Longest tool output kept in a failure message; the rest is cut from the top.
const MAX_OUTPUT_LINES: usize = 150;

/// Result of one verify step.
pub enum Outcome {
    /// The step passed; the text is an optional short detail ("3 крейта").
    Pass(String),
    /// The step failed; the text says what is wrong and how to fix it.
    Fail(String),
}

/// Runs `cargo <args>` and fails with its output and `hint` if it exits non-zero.
pub fn cargo(args: &[&str], hint: &str) -> Outcome {
    // `cargo xtask` sets CARGO to the cargo binary that started us.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let shown = format!("cargo {}", args.join(" "));
    match Command::new(&cargo).args(args).output() {
        Err(error) => Outcome::Fail(format!("не удалось запустить `{shown}`: {error}")),
        Ok(output) if output.status.success() => Outcome::Pass(String::new()),
        Ok(output) => {
            let text = tail(&combined(&output), MAX_OUTPUT_LINES);
            Outcome::Fail(format!("`{shown}` завершился с ошибкой:\n{text}\n→ {hint}"))
        }
    }
}

/// Runs `git <args>` and returns its stdout.
pub fn git(args: &[&str]) -> Result<String, String> {
    let shown = format!("git {}", args.join(" "));
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|error| format!("не удалось запустить `{shown}`: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`{shown}` завершился с ошибкой:\n{}",
            combined(&output)
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| format!("`{shown}` вернул не UTF-8"))
}

fn combined(output: &Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text.trim_end().to_owned()
}

/// Keeps the last `max` lines of `text`, noting how many were dropped.
fn tail(text: &str, max: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    match lines.len().checked_sub(max) {
        Some(dropped) if dropped > 0 => {
            let kept = lines.get(dropped..).unwrap_or_default().join("\n");
            format!("… (первые {dropped} строк вывода скрыты)\n{kept}")
        }
        _ => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::tail;

    #[test]
    fn tail_keeps_short_text() {
        assert_eq!(tail("a\nb", 5), "a\nb");
    }

    #[test]
    fn tail_cuts_from_the_top() {
        assert_eq!(
            tail("a\nb\nc\nd", 2),
            "… (первые 2 строк вывода скрыты)\nc\nd"
        );
    }
}
