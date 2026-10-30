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

/// Loads every `.ttf` and `.otf` file of `dir` (not its subfolders).
pub(crate) fn load(fonts: &mut FontSystem, dir: &Path) -> Result<(), RenderError> {
    let error = |source| RenderError::Fonts {
        path: dir.to_path_buf(),
        source,
    };
    for entry in std::fs::read_dir(dir).map_err(error)? {
        let path = entry.map_err(error)?.path();
        let font = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf"));
        if font {
            fonts
                .db_mut()
                .load_font_file(&path)
                .map_err(|source| RenderError::Fonts { path, source })?;
        }
    }
    Ok(())
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
