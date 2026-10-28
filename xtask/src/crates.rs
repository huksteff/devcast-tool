//! Every crate has a short `CLAUDE.md` and inherits the workspace lints (docs/ARCHITECTURE.md,
//! «Правила кода»): without `[lints] workspace = true` clippy does not enforce unwrap or length rules.

use std::path::Path;

use crate::metadata::Workspace;
use crate::run::Outcome;
use crate::sources;

pub const MAX_CRATE_DOC_LINES: usize = 40;
pub const MAX_ROOT_DOC_LINES: usize = 150;
const DOC: &str = "CLAUDE.md";

pub fn check(workspace: &Workspace) -> Outcome {
    let root = &workspace.root;
    let mut problems = Vec::new();
    match sources::read(&root.join(DOC)) {
        Ok(text) => problems.extend(doc_too_long(DOC, &text, MAX_ROOT_DOC_LINES)),
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
            Ok(text) => problems.extend(doc_too_long(
                &format!("{shown}/{DOC}"),
                &text,
                MAX_CRATE_DOC_LINES,
            )),
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

fn doc_too_long(shown: &str, text: &str, limit: usize) -> Option<String> {
    let lines = text.lines().count();
    (lines > limit).then(|| {
        format!(
            "{shown}: {lines} строк, лимит {limit}.\n  → Оставь инварианты и ссылки, подробности перенеси в docs/."
        )
    })
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
        let text = "строка\n".repeat(MAX_CRATE_DOC_LINES + 1);
        let message = doc_too_long("fx/zoom/CLAUDE.md", &text, MAX_CRATE_DOC_LINES).unwrap();
        assert!(
            message.starts_with("fx/zoom/CLAUDE.md: 41 строк"),
            "{message}"
        );
        assert!(doc_too_long("CLAUDE.md", &text, MAX_ROOT_DOC_LINES).is_none());
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
