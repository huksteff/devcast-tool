//! No effect code in the editor (task 7.35, docs/ARCHITECTURE.md «Панель эффекта», «Дорожка
//! эффекта»): the code of `crates/app/src` names an effect crate `dc_fx_*` only in the
//! registration files, and those re-export only an effect's seam modules (`lane`, `stage`,
//! `panel`, `dialog`), never a whole crate or its other items. Tests and comments do not count.

use std::path::Path;

use crate::run::Outcome;
use crate::sources;

/// Where the editor's code lives.
const APP: &str = "crates/app/src/";
/// Files that place the effects in the editor: which effect gives which row, panel, preview
/// gesture or dialog, and the recognition of captions (decision 018).
const REGISTRATION: [&str; 6] = [
    "crates/app/src/editor/lanes.rs",
    "crates/app/src/editor/inspector/mod.rs",
    "crates/app/src/editor/inspector/object.rs",
    "crates/app/src/editor/inspector/captions.rs",
    "crates/app/src/editor/crop.rs",
    "crates/app/src/editor/generation.rs",
];
/// Modules of an effect the editor works through.
const SEAMS: [&str; 4] = ["lane", "stage", "panel", "dialog"];
const EFFECT: &str = "dc_fx_";

pub fn check(root: &Path) -> Outcome {
    let files = match sources::rust_files(root) {
        Ok(files) => files,
        Err(error) => return Outcome::Fail(error),
    };
    let mut problems = Vec::new();
    for path in &files {
        let shown = sources::display(root, path);
        if !shown.starts_with(APP) || is_test(&shown) {
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
            "{}\n→ Поведение эффекта — в его крейте за швом (`lane`, `stage`, `panel`, `dialog`); редактор берёт его через реэкспорт в `editor/lanes.rs` (docs/ARCHITECTURE.md «Дорожка эффекта»).",
            problems.join("\n")
        ))
    }
}

pub fn is_test(shown: &str) -> bool {
    let name = shown.rsplit('/').next().unwrap_or(shown);
    name.ends_with("_tests.rs") || name == "tests.rs" || name == "test_project.rs"
}

fn find(shown: &str, text: &str) -> Vec<String> {
    let registration = REGISTRATION.contains(&shown);
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//") && line.contains(EFFECT))
        .filter_map(|(index, line)| {
            let at = format!("{shown}:{}", index + 1);
            if !registration {
                return Some(format!("{at}: крейт эффекта вне файлов регистрации"));
            }
            let reexport = line.trim_start().starts_with("pub use ");
            (reexport && !into_seam(line)).then(|| format!("{at}: реэкспорт не модуля шва эффекта"))
        })
        .collect()
}

/// `pub use dc_fx_zoom::lane as zoom;`: the path goes into a seam module of the effect.
fn into_seam(line: &str) -> bool {
    let Some((_, path)) = line.split_once(EFFECT) else {
        return false;
    };
    let mut parts = path.split("::");
    parts.next();
    let module = parts.next().unwrap_or_default();
    let module = module.split([' ', ';', ',']).next().unwrap_or_default();
    SEAMS.contains(&module)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_outside_registration_are_problems() {
        let text = "use dc_fx_zoom::ZoomId;\n// dc_fx_zoom in a comment\nfn a() {}\n";
        let found = find("crates/app/src/editor/view.rs", text);
        assert_eq!(found.len(), 1);
        assert!(found[0].starts_with("crates/app/src/editor/view.rs:1:"));
    }

    #[test]
    fn registration_reexports_only_seam_modules() {
        let text = "use dc_fx_annotations::Kind;\npub use dc_fx_zoom::lane as zoom;\npub use dc_fx_captions::panel::Phase;\npub use dc_fx_zoom as zoom;\npub use dc_fx_zoom::ZoomId;\n";
        let found = find("crates/app/src/editor/lanes.rs", text);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].contains(":4:") && found[1].contains(":5:"));
    }

    #[test]
    fn tests_do_not_count() {
        assert!(is_test("crates/app/src/editor/timeline/regions_tests.rs"));
        assert!(is_test("crates/app/src/editor/test_project.rs"));
        assert!(!is_test("crates/app/src/editor/regions.rs"));
    }
}
