//! Every crate has a short `CLAUDE.md` and inherits the workspace lints (docs/ARCHITECTURE.md,
//! «Правила кода»): without `[lints] workspace = true` clippy does not enforce unwrap or length rules.
//! A `CLAUDE.md` is read by every session in its crate, so it is capped in lines, characters
//! (a crate one at about 3k tokens) and characters per line (one invariant, not a dump).

use std::path::Path;

use crate::metadata::Workspace;
use crate::run::Outcome;
use crate::sources;

/// Lines and characters of a `CLAUDE.md`.
#[derive(Debug, Clone, Copy)]
pub struct DocLimit {
    pub lines: usize,
    pub chars: usize,
}

pub const CRATE_DOC: DocLimit = DocLimit {
    lines: 40,
    chars: 8000,
};
pub const ROOT_DOC: DocLimit = DocLimit {
    lines: 150,
    chars: 12000,
};
/// Characters of one line of any `CLAUDE.md`.
pub const MAX_DOC_LINE: usize = 1000;
const DOC: &str = "CLAUDE.md";

pub fn check(workspace: &Workspace) -> Outcome {
    let root = &workspace.root;
    let mut problems = Vec::new();
    match sources::read(&root.join(DOC)) {
        Ok(text) => problems.extend(doc_too_long(DOC, &text, ROOT_DOC)),
        Err(error) => problems.push(error),
    }
    for package in &workspace.packages {
        let dir = root.join(&package.dir);
        problems.extend(crate_problems(&package.dir, &dir));
    }
    if problems.is_empty() {
        Outcome::Pass(format!("{} крейтов", workspace.packages.len()))
    } else {
        Outcome::Fail(problems.join("\n"))
    }
}

fn crate_problems(shown: &str, dir: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    let doc = dir.join(DOC);
    if !doc.is_file() {
        problems.push(format!(
            "{shown}: нет {DOC}.\n  → Создай его по шаблону docs/templates/crate-CLAUDE.md."
        ));
    } else {
        match sources::read(&doc) {
            Ok(text) => problems.extend(doc_too_long(&format!("{shown}/{DOC}"), &text, CRATE_DOC)),
            Err(error) => problems.push(error),
        }
    }
    match sources::read(&dir.join("Cargo.toml")) {
        Ok(manifest) if !inherits_lints(&manifest) => problems.push(format!(
            "{shown}/Cargo.toml: нет `[lints] workspace = true`.\n  → Добавь секцию, иначе правила clippy workspace на крейт не действуют."
        )),
        Ok(_) => {}
        Err(error) => problems.push(error),
    }
    problems
}

fn doc_too_long(shown: &str, text: &str, limit: DocLimit) -> Vec<String> {
    const HINT: &str = "Оставь инварианты, нужные агенту, и ссылки; историю и «как сделано» убери: их хранят код и CHANGELOG.";
    let mut problems = Vec::new();
    let lines = text.lines().count();
    if lines > limit.lines {
        let max = limit.lines;
        problems.push(format!("{shown}: {lines} строк, лимит {max}.\n  → {HINT}"));
    }
    let chars = text.chars().count();
    if chars > limit.chars {
        let max = limit.chars;
        problems.push(format!(
            "{shown}: {chars} символов, лимит {max}.\n  → {HINT}"
        ));
    }
    for (index, line) in text.lines().enumerate() {
        let chars = line.chars().count();
        if chars > MAX_DOC_LINE {
            let number = index + 1;
            problems.push(format!(
                "{shown}:{number}: строка {chars} символов, лимит {MAX_DOC_LINE}.\n  → Один пункт — один инвариант: раздели или сократи. {HINT}"
            ));
        }
    }
    problems
}

fn inherits_lints(manifest: &str) -> bool {
    let mut lines = manifest.lines().map(str::trim);
    lines.any(|line| line == "[lints]") && lines.next() == Some("workspace = true")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_doc_is_reported_with_hint() {
        let text = "строка\n".repeat(CRATE_DOC.lines + 1);
        let problems = doc_too_long("fx/zoom/CLAUDE.md", &text, CRATE_DOC);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("fx/zoom/CLAUDE.md: 41 строк"));
        assert!(doc_too_long("CLAUDE.md", &text, ROOT_DOC).is_empty());
    }

    #[test]
    fn characters_and_long_lines_are_counted_not_bytes() {
        let line = "я".repeat(MAX_DOC_LINE);
        assert!(doc_too_long("a/CLAUDE.md", &line, CRATE_DOC).is_empty());
        let text = format!("{line}я\n");
        let problems = doc_too_long("a/CLAUDE.md", &text, CRATE_DOC);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("a/CLAUDE.md:1: строка 1001 символов"));
        let text = format!("{}\n", "я".repeat(900)).repeat(9);
        let problems = doc_too_long("a/CLAUDE.md", &text, CRATE_DOC);
        assert!(
            problems[0].contains("8109 символов, лимит 8000"),
            "{problems:?}"
        );
    }

    #[test]
    fn lints_must_be_inherited_from_the_workspace() {
        assert!(inherits_lints(
            "[package]\nname = \"a\"\n\n[lints]\nworkspace = true\n"
        ));
        assert!(!inherits_lints("[package]\nname = \"a\"\n"));
        assert!(!inherits_lints("[lints]\n\n[lints.clippy]\n"));
    }
}
