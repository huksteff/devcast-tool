//! Which project the editor holds (7.28): opening one (after a recording, from the «Проекты»
//! menu, a new one), another in the same window, the copy of «Сохранить как…» and renaming. The
//! editor that gives way writes its edits not on disk yet to its session and is dropped with its
//! threads, which end before the drop returns: Windows renames no folder with a file open in it.

use std::path::Path;

use dc_core::project::MediaKind;
use dc_project::ProjectDir;
use dc_ui_kit::strings::projects as text;

use super::projects::RECENT_MAX;
use super::{Document, EditorWindow, LOCK_OWNER, Leave, timeline_window};
use crate::app::Shared;

impl Shared {
    /// Opens the project in `root` in the editor and hides the HUD; the editor open before gives
    /// way. With `recover` a working state left on disk is asked about. A project that cannot be
    /// opened (another writer holds it) is a toast and the open editor stays.
    pub fn open_project(&mut self, root: &Path, recover: bool) -> bool {
        let opened = self.try_open(root, recover);
        let done = opened.is_ok();
        self.report(opened);
        done
    }

    /// [`Self::open_project`] telling why the project did not open instead of a toast.
    fn try_open(&mut self, root: &Path, recover: bool) -> Result<(), String> {
        let opened = ProjectDir::open(root)
            .map_err(super::DocumentError::from)
            .and_then(|dir| EditorWindow::open(dir, Vec::new()));
        let (mut editor, problems) = opened.map_err(|e| e.to_string())?;
        self.drop_editor();
        editor.sound = recorded_sound(&editor.doc);
        editor.retrack();
        editor.recovery = recover && editor.doc.unsaved();
        self.start_editor(&mut editor);
        for problem in problems {
            self.report(Err(problem));
        }
        timeline_window::restore(&mut editor, &self.store.get().editor);
        let root = editor.doc.dir().root().to_path_buf();
        self.editor = Some(editor);
        self.remember_project(&root, None);
        self.hide_hud();
        Ok(())
    }

    /// `devcast open`: the editor opens the project in `root`; with unsaved changes of another
    /// project it asks in its window first (`Answer::Asking`).
    pub fn open_asked(&mut self, root: &Path) -> dc_ops::live::Answer {
        use dc_ops::live::Answer;
        if let Some(editor) = self.editor.as_mut() {
            if super::live::same_folder(editor.doc.dir().root(), root) {
                return Answer::Opened;
            }
            if editor.doc.unsaved() {
                editor.leave(Leave::Open(root.to_path_buf()));
                editor.detached.wake_editor = true;
                return Answer::Asking;
            }
        }
        match self.try_open(root, true) {
            Ok(()) => Answer::Opened,
            Err(error) => Answer::Refused(error),
        }
    }

    /// Does what the editor asked after its frame.
    pub fn leave_project(&mut self, leave: Leave) {
        match leave {
            Leave::Close => {
                self.drop_editor();
                self.show_hud();
            }
            Leave::New => self.new_project(),
            Leave::Open(root) => {
                let open = self.editor.as_ref().map(|e| e.doc.dir().root());
                if open.is_some_and(|open| super::live::same_folder(open, &root)) {
                    return;
                }
                if root.extension().is_none_or(|e| e != "devcast") {
                    self.report(Err(text::NOT_A_PROJECT));
                    return;
                }
                self.open_project(&root, true);
            }
            Leave::Copied(root) => {
                if let Some(editor) = self.editor.as_mut() {
                    let discarded = editor.doc.discard();
                    self.report(discarded);
                }
                self.open_project(&root, false);
            }
            Leave::Rename(name) => self.rename_project(&name),
        }
    }

    /// The editor gives way: its edits not on disk go to its session, its threads end.
    fn drop_editor(&mut self) {
        timeline_window::remember(self);
        let Some(mut editor) = self.editor.take() else {
            return;
        };
        let flushed = editor.doc.flush();
        self.report(flushed);
    }

    /// «Новый проект»: an empty project in the recordings folder, with the defaults.
    fn new_project(&mut self) {
        let folder = self.store.get().recordings_dir.clone();
        let root = (1..)
            .map(|n| folder.join(format!("{}.devcast", text::new_name(n))))
            .find(|root| !root.exists())
            .unwrap_or_else(|| folder.join(text::NEW));
        let made = std::fs::create_dir_all(&folder)
            .map_err(|e| e.to_string())
            .and_then(|()| {
                let defaults = dc_project::settings::defaults_file()
                    .and_then(|file| dc_project::Defaults::load(&file))
                    .map_err(|e| e.to_string())?;
                dc_ops::new_project(&root, &[], &defaults, LOCK_OWNER).map_err(|e| e.to_string())
            });
        match made {
            Ok(_) => {
                self.open_project(&root, false);
            }
            Err(error) => self.report(Err(error)),
        }
    }

    /// Renames the project folder and opens it again with its working state and history. Waits
    /// for no export: one running reads the project, so renaming is refused meanwhile.
    fn rename_project(&mut self, name: &str) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        if self
            .exports
            .as_ref()
            .is_some_and(crate::exports::Exports::busy)
        {
            self.report(Err(text::RENAME_EXPORTING));
            return;
        }
        let old = editor.doc.dir().root().to_path_buf();
        let checked = editor.doc.dir().renamed_root(name).map(drop);
        let flushed = checked.and_then(|()| editor.doc.flush());
        if let Err(error) = flushed {
            self.report(Err(error));
            return;
        }
        timeline_window::remember(self);
        let Some(editor) = self.editor.take() else {
            return;
        };
        let (dir, lock) = editor.doc_only().into_parts();
        let renamed = dir
            .rename(lock, name)
            .map(|renamed| renamed.root().to_path_buf());
        let root = match renamed {
            Ok(root) => root,
            Err(error) => {
                self.report(Err(error));
                old.clone()
            }
        };
        if self.open_project(&root, false) {
            self.remember_project(&root, Some(&old));
        }
    }

    /// Puts `root` first in the recent projects, without `replaced` (the old folder of a renamed
    /// one).
    fn remember_project(&mut self, root: &Path, replaced: Option<&Path>) {
        let root = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
        let saved = self.store.update(|s| {
            s.recent_projects
                .retain(|p| *p != root && replaced.is_none_or(|old| p != old));
            s.recent_projects.insert(0, root.clone());
            s.recent_projects.truncate(RECENT_MAX);
        });
        self.report(saved);
    }
}

/// The sound tracks of the recordings of the project (`mic`, `system`), each once: the rows of
/// the source sound. A recording whose manifest cannot be read adds none (logged).
fn recorded_sound(doc: &Document) -> Vec<String> {
    let mut sound: Vec<String> = Vec::new();
    let recordings = doc.project().clips.iter().map(|c| &c.media);
    for media in recordings.filter(|m| m.kind == MediaKind::Recording) {
        let folder = dc_capture::project::recording_dir(doc.dir().root(), media);
        match dc_capture::Manifest::read(&folder) {
            Ok(manifest) => {
                let names = manifest.tracks.into_iter().map(|t| t.name);
                for name in names.filter(|n| n != dc_capture::VIDEO_TRACK) {
                    if !sound.contains(&name) {
                        sound.push(name);
                    }
                }
            }
            Err(error) => {
                tracing::warn!("дорожки записи {} не прочитаны: {error}", folder.display())
            }
        }
    }
    sound
}

impl EditorWindow {
    /// The document alone; everything else of the window (its threads too) is dropped first.
    fn doc_only(self) -> Document {
        self.doc
    }
}

#[cfg(test)]
#[path = "switch_tests.rs"]
mod tests;
