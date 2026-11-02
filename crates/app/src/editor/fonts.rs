//! The fonts of the open project for the font pickers of the text annotation and the captions:
//! the built-in families and those added to the project (`dc_ops::fonts`). «Добавить шрифт…»
//! copies a file into the project as `devcast font add` does and chooses its family with the
//! panel's own command, one undo step; the copy stays, like an imported picture.

use std::path::PathBuf;
use std::time::Instant;

use dc_ops::Step;
use dc_project::ProjectDir;
use serde_json::json;

use super::EditorWindow;

/// A font file to add and the command of the panel that chooses it: its `font` argument is the
/// family of the file.
#[derive(Debug, Clone, PartialEq)]
pub struct FontFile {
    pub path: PathBuf,
    pub then: Step,
}

/// The families the pickers of the project in `dir` list.
pub fn families(dir: &ProjectDir) -> Result<Vec<String>, String> {
    dc_ops::fonts::families(dir.root()).map_err(|e| e.to_string())
}

/// Adds the font `file` to the project and chooses it; whether the project changed, or what
/// failed.
pub fn add(editor: &mut EditorWindow, file: FontFile, now: Instant) -> Result<bool, String> {
    let imported =
        dc_ops::import_font(editor.doc.dir(), &file.path, false).map_err(|e| e.to_string())?;
    editor.fonts = families(editor.doc.dir())?;
    let mut step = file.then;
    step.args.insert("font".to_owned(), json!(imported.family));
    let applied = editor.doc.apply(&[step], now).map_err(|e| e.to_string())?;
    Ok(applied.changed)
}
