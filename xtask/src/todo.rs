//! Unfinished-work markers are not allowed in committed code (docs/PROCESS.md, «без костылей»).

use std::path::Path;

use crate::run::Outcome;
use crate::sources;

/// Marker words, assembled from parts so this file does not flag itself.
fn markers() -> [String; 4] {
    [
        ["TO", "DO"].concat(),
        ["FIX", "ME"].concat(),
        ["XX", "X"].concat(),
        ["HA", "CK"].concat(),
    ]
}

pub fn check(root: &Path) -> Outcome {
    let files = match sources::rust_files(root) {
        Ok(files) => files,
        Err(error) => return Outcome::Fail(error),
    };
    let mut problems = Vec::new();
    for path in &files {
        match sources::read(path) {
            Ok(text) => problems.extend(find(&sources::display(root, path), &text)),
            Err(error) => problems.push(error),
        }
    }
    if problems.is_empty() {
        Outcome::Pass(String::new())
    } else {
        Outcome::Fail(format!(
            "{}\n→ Доделай сейчас. Если это отдельная работа — добавь задачу в docs/roadmap и убери пометку.",
            problems.join("\n")
        ))
    }
}

fn find(shown: &str, text: &str) -> Vec<String> {
    let markers = markers();
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let marker = markers.iter().find(|m| contains_word(line, m))?;
            Some(format!("{shown}:{}: пометка {marker} запрещена", index + 1))
        })
        .collect()
}

/// True if `word` occurs in `line` not glued to letters, digits or `_` on either side.
fn contains_word(line: &str, word: &str) -> bool {
    let is_word_char = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices(word).any(|(start, _)| {
        let before = line.get(..start).and_then(|s| s.chars().next_back());
        let after = line
            .get(start + word.len()..)
            .and_then(|s| s.chars().next());
        !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_marker_in_comment() {
        let text = format!("fn a() {{}}\n// {}: later\n", ["TO", "DO"].concat());
        let found = find("x.rs", &text);
        assert_eq!(found.len(), 1);
        assert!(found[0].starts_with("x.rs:2:"));
    }

    #[test]
    fn ignores_marker_inside_identifiers() {
        let text = format!("let {}_count = 1;\n", ["to", "do"].concat().to_uppercase());
        assert!(find("x.rs", &text).is_empty());
        assert!(find("x.rs", "let todos = 1; // методы TODOLIST\n").is_empty());
    }

    #[test]
    fn ignores_clean_code() {
        assert!(find("x.rs", "fn main() {}\n").is_empty());
    }
}
