//! No command named by text in the editor and the effects: a button asks for a command by the
//! name of its type (`Step::of(OpName::of::<ZoomSet>(), …)`, `dc_core::step`), so a renamed or
//! removed command fails the build, not the click. `Step::new` (a name as text) stays for the
//! lines of `devcast` and the plan files. Tests and comments do not count.

use std::path::Path;

use crate::run::Outcome;
use crate::{seams, sources};

/// Where the buttons live: the effects, the editor and the kit of its panels.
const PLACES: [&str; 3] = ["fx/", "crates/app/src/", "crates/ui-kit/src/"];
const BY_TEXT: &str = "Step::new(";

pub fn check(root: &Path) -> Outcome {
    let files = match sources::rust_files(root) {
        Ok(files) => files,
        Err(error) => return Outcome::Fail(error),
    };
    let mut problems = Vec::new();
    for path in &files {
        let shown = sources::display(root, path);
        if !PLACES.iter().any(|place| shown.starts_with(place)) || seams::is_test(&shown) {
            continue;
        }
        match sources::read(path) {
            Ok(text) => problems.extend(find(&shown, &text)),
            Err(error) => problems.push(error),
        }
    }
    if problems.is_empty() {
        Outcome::Pass(String::new())
    } else {
        Outcome::Fail(format!(
            "{}\n→ Назови команду её типом: `const SET: OpName = OpName::of::<ZoomSet>();` и `Step::of(SET, args)` — переименование команды тогда ломает сборку, а не кнопку.",
            problems.join("\n")
        ))
    }
}

fn find(shown: &str, text: &str) -> Vec<String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//") && line.contains(BY_TEXT))
        .map(|(index, _)| format!("{shown}:{}: команда названа текстом", index + 1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_named_by_text_is_a_problem_and_a_comment_is_not() {
        let text = "// Step::new(\"zoom set\", args)\nlet a = Step::of(SET, args);\nlet b = Step::new(\"zoom set\", args);\n";
        let found = find("fx/zoom/src/lane.rs", text);
        assert_eq!(found, ["fx/zoom/src/lane.rs:3: команда названа текстом"]);
    }
}
