//! Fonts of a project, one way for `export`, `devcast frame`/`sheet`, the editor preview and the
//! font pickers: the built-in fonts next to the program and the project's own `.ttf`/`.otf`
//! files in `assets/fonts/` (decision 005). `devcast font add` and «Добавить шрифт…» of the
//! editor copy a file there (`import_font`); a style names the font by its family.

use std::path::{Path, PathBuf};

use dc_project::{CopiedFile, ProjectDir, ProjectError};
use dc_render::{FAMILIES, RenderError};

/// Folder of the project's own fonts, a path with `/` from the project root.
pub const FONTS_DIR: &str = "assets/fonts";

#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("«{}» — не файл шрифта: укажи файл .ttf или .otf", path.display())]
    NotAFontFile { path: PathBuf },
    #[error("файл «{}» не найден: проверь путь", path.display())]
    NotFound { path: PathBuf },
    #[error("шрифт «{family}» уже встроен в программу: выбери его в стиле без добавления")]
    BuiltIn { family: String },
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error(transparent)]
    Project(#[from] ProjectError),
}

/// A font file put into the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontImport {
    /// The family a style names it by (English name from the file).
    pub family: String,
    pub file: CopiedFile,
}

/// Loads the fonts of the project in `root` with `load` (`TextMeasure::load_fonts`,
/// `Compositor::load_fonts`): the built-in ones, then `assets/fonts/` when the project has it.
/// Loading again adds only the files that are new.
pub fn load(
    root: &Path,
    mut load: impl FnMut(&Path) -> Result<(), RenderError>,
) -> Result<(), RenderError> {
    load(&dc_render::builtin_fonts_dir()?)?;
    let own = dir(root);
    if own.is_dir() {
        load(&own)?;
    }
    Ok(())
}

/// The families a style of the project in `root` can name: the built-in ones in their order,
/// then the project's own, sorted.
pub fn families(root: &Path) -> Result<Vec<String>, RenderError> {
    let mut all: Vec<String> = FAMILIES.iter().map(|f| (*f).to_owned()).collect();
    let own = dir(root);
    if own.is_dir() {
        let new: Vec<String> = dc_render::families_in(&own)?
            .into_iter()
            .filter(|f| !all.contains(f))
            .collect();
        all.extend(new);
    }
    Ok(all)
}

/// The project's own fonts: each family and its file in the project, by file name.
pub fn project_fonts(root: &Path) -> Result<Vec<(String, String)>, FontError> {
    let own = dir(root);
    if !own.is_dir() {
        return Ok(Vec::new());
    }
    let error = |source| RenderError::Fonts {
        path: own.clone(),
        source,
    };
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&own).map_err(error)? {
        let path = entry.map_err(error)?.path();
        if dc_render::is_font(&path) {
            files.push(path);
        }
    }
    files.sort();
    let mut fonts = Vec::new();
    for path in files {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        let file = format!("{FONTS_DIR}/{}", name.unwrap_or_default());
        for family in dc_render::file_families(&path)? {
            fonts.push((family, file.clone()));
        }
    }
    Ok(fonts)
}

/// Copies the font file `source` into `assets/fonts/` of the project in `dir` (not with
/// `dry_run`); the same file added again is not copied twice.
pub fn import_font(
    dir: &ProjectDir,
    source: &Path,
    dry_run: bool,
) -> Result<FontImport, FontError> {
    let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned());
    let (Some(stem), true) = (stem, dc_render::is_font(source)) else {
        return Err(FontError::NotAFontFile {
            path: source.to_path_buf(),
        });
    };
    if !source.is_file() {
        return Err(FontError::NotFound {
            path: source.to_path_buf(),
        });
    }
    let families = dc_render::file_families(source)?;
    if let Some(family) = families
        .iter()
        .find(|f| FAMILIES.iter().any(|b| b.eq_ignore_ascii_case(f)))
    {
        return Err(FontError::BuiltIn {
            family: family.clone(),
        });
    }
    let extension = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let file = dc_project::copy_in(dir, FONTS_DIR, source, &stem, &extension, dry_run)?;
    let family = families.into_iter().next().unwrap_or_default();
    Ok(FontImport { family, file })
}

fn dir(root: &Path) -> PathBuf {
    FONTS_DIR
        .split('/')
        .fold(root.to_path_buf(), |d, p| d.join(p))
}
