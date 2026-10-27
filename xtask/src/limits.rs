//! File size limit. The function limit (80 lines) is checked by clippy's `too_many_lines`.

use std::path::Path;

use crate::run::Outcome;
use crate::sources;

/// Maximum number of lines in one `.rs` file (docs/ARCHITECTURE.md, «Правила кода»).
pub const MAX_FILE_LINES: usize = 400;

pub fn check(root: &Path) -> Outcome {
    let files = match sources::rust_files(root) {
        Ok(files) => files,
        Err(error) => return Outcome::Fail(error),
    };
    let mut problems = Vec::new();
    for path in &files {
        match sources::read(path) {
            Ok(text) => problems.extend(too_long(&sources::display(root, path), &text)),
            Err(error) => problems.push(error),
        }
    }
    if problems.is_empty() {
        Outcome::Pass(format!("{} файлов", files.len()))
    } else {
        Outcome::Fail(problems.join("\n"))
    }
}

fn too_long(shown: &str, text: &str) -> Option<String> {
    let lines = text.lines().count();
    (lines > MAX_FILE_LINES).then(|| {
        format!(
            "{shown}: {lines} строк, лимит {MAX_FILE_LINES}.\n  → Раздели файл по смыслу на модули (каждый со своей ответственностью), а не механически пополам."
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_file_passes() {
        assert!(too_long("a.rs", "fn main() {}\n").is_none());
    }

    #[test]
    fn long_file_is_reported_with_hint() {
        let text = "// line\n".repeat(MAX_FILE_LINES + 1);
        let message = too_long("crates/x/src/big.rs", &text).unwrap();
        assert!(message.starts_with("crates/x/src/big.rs: 401 строк"));
        assert!(message.contains("Раздели файл"));
    }

    #[test]
    fn exactly_at_limit_passes() {
        assert!(too_long("a.rs", &"x\n".repeat(MAX_FILE_LINES)).is_none());
    }
}
