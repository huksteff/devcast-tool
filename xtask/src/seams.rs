//! No effect code in the editor (task 7.35, docs/ARCHITECTURE.md «Панель эффекта», «Дорожка
//! эффекта»): the code of `crates/app/src` names an effect crate `dc_fx_*` only in the
//! registration files, and those take only an effect's seam modules (`lane`, `stage`,
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
            "{}\n→ Поведение эффекта — в его крейте за швом (`lane`, `stage`, `panel`, `dialog`); файлы регистрации (`editor/lanes.rs`, `inspector/`) берут у эффекта только эти модули: команду, вид или факт эффект отдаёт из своего шва (`dc_fx_cut::lane::CUT`), docs/ARCHITECTURE.md «Дорожка эффекта».",
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
            (!only_seams(line)).then(|| format!("{at}: у эффекта берётся не модуль шва"))
        })
        .collect()
}

/// Every effect path of `line` goes into a seam module of the effect: `dc_fx_zoom::lane`,
/// `dc_fx_zoom::stage::ZoomFrame`; not the crate itself, a braced list or another item.
fn only_seams(line: &str) -> bool {
    line.split(EFFECT).skip(1).all(|path| {
        let name_end = path
            .find(|c: char| !(c.is_ascii_lowercase() || c == '_'))
            .unwrap_or(path.len());
        let Some(rest) = path[name_end..].strip_prefix("::") else {
            return false;
        };
        let module_end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        SEAMS.contains(&&rest[..module_end])
    })
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
    fn registration_takes_only_seam_modules() {
        let lines = [
            "pub use dc_fx_zoom::lane as zoom;",
            "pub use dc_fx_captions::panel::Phase;",
            "    &dc_fx_zoom::stage::ZoomFrame, &dc_fx_drawing::stage::Strokes,",
            "use dc_fx_annotations::Kind;",
            "pub use dc_fx_zoom as zoom;",
            "use dc_fx_cut::{lane, Cut};",
            "const CUT: OpName = OpName::of::<dc_fx_cut::Cut>();",
            "    &dc_fx_zoom::stage::ZoomFrame, &dc_fx_text::TextBody,",
        ];
        let found = find("crates/app/src/editor/lanes.rs", &lines.join("\n"));
        let at: Vec<_> = found.iter().map(|f| f.split(':').nth(1)).collect();
        let wrong = ["4", "5", "6", "7", "8"].map(Some);
        assert_eq!(at, wrong, "{found:?}");
    }

    #[test]
    fn tests_do_not_count() {
        assert!(is_test("crates/app/src/editor/timeline/regions_tests.rs"));
        assert!(is_test("crates/app/src/editor/test_project.rs"));
        assert!(!is_test("crates/app/src/editor/regions.rs"));
    }
}
