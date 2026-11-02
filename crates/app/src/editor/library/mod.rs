//! The recordings library (7.27, mockup 6 «Записи»): the projects of the recordings folder of
//! the settings (`scan.rs`) in a 300 px panel over the left of the preview (`panel.rs`), opened
//! by «Записи» of the header and closed by it or Esc. A recording goes onto the timeline by its
//! +, a double click, «На таймлайн» or a drag onto the timeline: `dc_ops::import_clips`, the path
//! of `devcast clip add <project>` (its recording folders are copied with cursor, keys and sound),
//! one undo step. «Импорт…» makes a project of each video in the recordings folder, the path of
//! `devcast new --each` (`dc_ops::new_projects`). The bin moves projects to the Recycle Bin
//! (`bin.rs`, the path of `devcast trash`); «Отменить» or Ctrl+Z while the panel is open brings
//! back the last batch.

mod bin;
mod panel;
mod scan;
mod thumbs;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::time::Instant;

use dc_fx_cut::ClipAdd;
use eframe::egui::{self, Rect, TextureHandle, Ui};

use super::{Action, EditorWindow, LOCK_OWNER};
pub use panel::show;
pub use scan::Entry;

/// The library panel and what it shows.
#[derive(Debug, Default)]
pub struct Library {
    pub open: bool,
    /// The recordings folder of the settings.
    folder: PathBuf,
    pub entries: Vec<Entry>,
    pub search: String,
    /// Picked recordings (their project folders), in the order picked.
    pub picked: Vec<PathBuf>,
    /// Where Shift+click picks from.
    anchor: Option<PathBuf>,
    /// The last batch moved to the Recycle Bin.
    pub trashed: Vec<PathBuf>,
    pub thumbs: thumbs::Thumbs,
    textures: Textures,
}

/// The textures of the tile pictures that came, by project folder.
#[derive(Default)]
struct Textures(Vec<(PathBuf, TextureHandle)>);

impl std::fmt::Debug for Textures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Textures({})", self.0.len())
    }
}

/// What the library asks of the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// These recordings onto the end of the timeline, as one undo step.
    Add(Vec<PathBuf>),
    /// A project of each video in the recordings folder.
    Import(Vec<PathBuf>),
    Trash(Vec<PathBuf>),
    /// The last batch back from the Recycle Bin.
    Restore,
    OpenFolder,
}

/// The recordings dragged from the library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dragged(pub Vec<PathBuf>);

impl Library {
    /// Starts the thread of the tile pictures; without it the tiles stay plain.
    pub fn start(&mut self, ctx: egui::Context) -> Result<(), crate::AppError> {
        self.thumbs
            .start(ctx)
            .map_err(|source| crate::AppError::Thread {
                name: "devcast-thumbs",
                source,
            })
    }

    /// The recordings folder of the settings; the list is read again when it changed.
    pub fn folder(&mut self, folder: &Path, open: &Path) {
        if self.folder != folder {
            self.folder = folder.to_path_buf();
            self.rescan(open);
        }
    }

    /// Opens the panel with a fresh list, or closes it.
    pub fn toggle(&mut self, open: &Path) {
        self.open = !self.open;
        if self.open {
            self.rescan(open);
        }
    }

    /// Reads the list again, keeps the picks that are still there and asks for the pictures.
    fn rescan(&mut self, open: &Path) {
        self.entries = scan::scan(&self.folder, open);
        let entries = &self.entries;
        self.picked.retain(|p| entries.iter().any(|e| e.root == *p));
        let wanted: Vec<thumbs::Thumb> = entries
            .iter()
            .map(|e| thumbs::Thumb(e.root.clone()))
            .collect();
        self.thumbs.want(wanted.iter());
    }

    /// The entries whose name has the search text, ignoring case.
    pub fn shown(&self) -> Vec<&Entry> {
        let search = self.search.trim().to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.name.to_lowercase().contains(&search))
            .collect()
    }

    /// Asks for the pictures of these projects (the menu of recent projects shows them too).
    pub fn want_pictures<'a>(&mut self, roots: impl Iterator<Item = &'a PathBuf>) {
        let wanted: Vec<thumbs::Thumb> = roots.map(|r| thumbs::Thumb(r.clone())).collect();
        self.thumbs.want(wanted.iter());
    }

    /// The texture of the picture of the project in `root`, made once it came.
    pub fn picture(&mut self, ctx: &egui::Context, root: &Path) -> Option<TextureHandle> {
        if let Some((_, texture)) = self.textures.0.iter().find(|(r, _)| r == root) {
            return Some(texture.clone());
        }
        let image = self.thumbs.get(&thumbs::Thumb(root.to_path_buf()))?;
        let name = root.display().to_string();
        let texture = ctx.load_texture(name, image.clone(), egui::TextureOptions::LINEAR);
        self.textures.0.push((root.to_path_buf(), texture.clone()));
        Some(texture)
    }

    /// Ctrl+Z goes to the Recycle Bin first while the panel is open and has a batch there.
    pub fn takes_undo(&self) -> bool {
        self.open && !self.trashed.is_empty()
    }
}

/// Does `ask` at `now`; true when the project changed.
pub fn run(editor: &mut EditorWindow, ask: Ask, now: Instant) -> Result<bool, String> {
    let open = editor.doc.dir().root().to_path_buf();
    let library = &mut editor.library;
    match ask {
        Ask::Add(roots) => return add(editor, &roots, now),
        Ask::Import(videos) => {
            let made = import(&library.folder, &videos, &editor.defaults);
            library.rescan(&open);
            library.picked = made?;
        }
        Ask::Trash(roots) => {
            bin::trash(&roots)?;
            library.trashed = roots;
            library.rescan(&open);
        }
        Ask::Restore => {
            let roots = std::mem::take(&mut library.trashed);
            let restored = bin::restore(&roots);
            library.rescan(&open);
            restored?;
            library.picked = roots;
        }
        Ask::OpenFolder => crate::open_folder(&library.folder).map_err(|e| e.to_string())?,
    }
    Ok(false)
}

/// `clip add` of `roots` as one undo step; refused, the copies are deleted.
fn add(editor: &mut EditorWindow, roots: &[PathBuf], now: Instant) -> Result<bool, String> {
    let import = dc_ops::import_clips(editor.doc.dir(), roots, false).map_err(|e| e.to_string())?;
    let commands = dc_ops::as_commands(&import.commands);
    match editor.doc.apply_commands(ClipAdd::NAME, &commands, now) {
        Ok(_) => Ok(true),
        Err(error) => Err(match import.discard() {
            Ok(()) => error.to_string(),
            Err(e) => format!("{error}; {e}"),
        }),
    }
}

/// A project `<video name>.devcast` (`-2`, `-3` … when taken) of each of `videos` in `folder`,
/// the path of `devcast new --each`.
fn import(
    folder: &Path,
    videos: &[PathBuf],
    defaults: &dc_project::Defaults,
) -> Result<Vec<PathBuf>, String> {
    let made = dc_ops::new_projects(folder, videos, defaults, LOCK_OWNER);
    let made = made.map_err(|e| e.to_string())?;
    Ok(made.into_iter().map(|(root, _)| root).collect())
}

/// Recordings dragged from the library and let go over the timeline `rect` go onto it.
pub fn drop_on(ui: &Ui, editor: &mut EditorWindow, rect: Rect) {
    let released = ui.input(|i| {
        i.pointer.any_released() && i.pointer.latest_pos().is_some_and(|p| rect.contains(p))
    });
    if released && let Some(dragged) = egui::DragAndDrop::take_payload::<Dragged>(ui.ctx()) {
        editor.ask(Action::Library(Ask::Add(dragged.0.clone())));
    }
}
