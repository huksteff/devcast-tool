//! The font system of the text pass and of [`super::TextMeasure`]: only the fonts loaded from our
//! folders (decision 005), so a frame and a measured width are the same on every PC.

use std::path::{Path, PathBuf};

use glyphon::{Attrs, Family, FontSystem, Style, Weight, fontdb};

use crate::error::RenderError;
use crate::scene::Font;

/// Folder of the built-in fonts next to the program.
pub const BUILTIN_FONTS: &str = "fonts";

/// An empty font system; Russian is the locale for picking fallback faces.
pub(crate) fn font_system() -> FontSystem {
    FontSystem::new_with_locale_and_db("ru-RU".into(), fontdb::Database::new())
}

/// Loads every `.ttf` and `.otf` file of `dir` (not its subfolders); a file loaded before is not
/// loaded again, so loading a folder again only adds its new files.
pub(crate) fn load(fonts: &mut FontSystem, dir: &Path) -> Result<(), RenderError> {
    let error = |source| RenderError::Fonts {
        path: dir.to_path_buf(),
        source,
    };
    for entry in std::fs::read_dir(dir).map_err(error)? {
        let path = entry.map_err(error)?.path();
        let loaded = fonts
            .db()
            .faces()
            .any(|face| matches!(&face.source, fontdb::Source::File(file) if *file == path));
        if is_font(&path) && !loaded {
            fonts
                .db_mut()
                .load_font_file(&path)
                .map_err(|source| RenderError::Fonts { path, source })?;
        }
    }
    Ok(())
}

/// Whether `path` names a `.ttf` or `.otf` file.
pub fn is_font(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| FONT_EXTENSIONS.iter().any(|f| e.eq_ignore_ascii_case(f)))
}

/// Extensions of the font files the text pass loads.
pub const FONT_EXTENSIONS: [&str; 2] = ["ttf", "otf"];

/// Families of the fonts in `dir` as the text pass loads them (English names), sorted.
pub fn families_in(dir: &Path) -> Result<Vec<String>, RenderError> {
    let mut fonts = font_system();
    load(&mut fonts, dir)?;
    Ok(names(fonts.db()))
}

/// Families of the font file `path` (English names), sorted; [`RenderError::NotAFont`] when it
/// holds no font the text pass can use.
pub fn file_families(path: &Path) -> Result<Vec<String>, RenderError> {
    let data = std::fs::read(path).map_err(|source| RenderError::Fonts {
        path: path.to_path_buf(),
        source,
    })?;
    let mut db = fontdb::Database::new();
    db.load_font_data(data);
    let families = names(&db);
    if families.is_empty() {
        return Err(RenderError::NotAFont {
            path: path.to_path_buf(),
        });
    }
    Ok(families)
}

/// The first (English) family name of every face, sorted, without repeats.
fn names(db: &fontdb::Database) -> Vec<String> {
    let mut names: Vec<String> = db
        .faces()
        .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The `fonts` folder next to the running program (`cargo xtask deps` puts the built-in fonts
/// next to the built programs and tests).
pub fn builtin_dir() -> Result<PathBuf, RenderError> {
    let exe = std::env::current_exe().map_err(RenderError::NoBuiltinFonts)?;
    Ok(exe.parent().unwrap_or(&exe).join(BUILTIN_FONTS))
}

pub(crate) fn check_family(fonts: &FontSystem, family: &str) -> Result<(), RenderError> {
    let loaded = fonts
        .db()
        .faces()
        .any(|face| face.families.iter().any(|(name, _)| name == family));
    if loaded {
        Ok(())
    } else {
        Err(RenderError::NoFont {
            family: family.to_owned(),
        })
    }
}

pub(crate) fn attrs(font: &Font) -> Attrs<'_> {
    Attrs::new()
        .family(Family::Name(&font.family))
        .weight(if font.bold {
            Weight::BOLD
        } else {
            Weight::NORMAL
        })
        .style(if font.italic {
            Style::Italic
        } else {
            Style::Normal
        })
}
