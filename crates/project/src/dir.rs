//! The project folder `name.dcproj/`:
//!
//! ```text
//! project.json    saved state (Ctrl+S)
//! session.json    working state: autosave, restored after a crash; absent = nothing unsaved
//! history.jsonl   undo/redo log
//! media/ assets/  recordings, imported files, fonts, images, cursors
//! cache/          derived data, safe to delete
//! ```

use std::path::{Path, PathBuf};

use dc_core::Project;
use serde_json::Value;

use crate::atomic;
use crate::error::{ProjectError, Result};
use crate::migrate::{MigrateError, migrate};

pub const PROJECT_FILE: &str = "project.json";
pub const SESSION_FILE: &str = "session.json";
pub const HISTORY_FILE: &str = "history.jsonl";
const FOLDERS: [&str; 3] = ["media", "assets", "cache"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDir {
    root: PathBuf,
}

impl ProjectDir {
    /// Creates a new folder with an empty project. Fails if the folder already exists.
    pub fn create(root: impl Into<PathBuf>) -> Result<Self> {
        let dir = Self { root: root.into() };
        if dir.root.exists() {
            return Err(ProjectError::AlreadyExists { path: dir.root });
        }
        for folder in FOLDERS {
            let path = dir.root.join(folder);
            std::fs::create_dir_all(&path).map_err(ProjectError::io("создать папку", &path))?;
        }
        atomic::write_json(&dir.file(PROJECT_FILE), &Project::default())?;
        Ok(dir)
    }

    /// Opens an existing project folder (does not read the project yet).
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let dir = Self { root: root.into() };
        if !dir.file(PROJECT_FILE).is_file() {
            return Err(ProjectError::NotAProject { path: dir.root });
        }
        Ok(dir)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn file(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn media_dir(&self) -> PathBuf {
        self.root.join("media")
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// The saved project (`project.json`), migrated and validated.
    pub fn load(&self) -> Result<Project> {
        read_project(&self.file(PROJECT_FILE))
    }

    /// Unsaved working state left by autosave or a crash, if any.
    pub fn load_session(&self) -> Result<Option<Project>> {
        let path = self.file(SESSION_FILE);
        if path.is_file() {
            read_project(&path).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.file(SESSION_FILE).is_file()
    }

    /// Ctrl+S: writes `project.json`; the working state is now saved, so `session.json` goes away.
    pub fn save(&self, project: &Project) -> Result<()> {
        check(project)?;
        atomic::write_json(&self.file(PROJECT_FILE), project)?;
        self.discard_session()
    }

    /// Autosave of the working state.
    pub fn save_session(&self, project: &Project) -> Result<()> {
        check(project)?;
        atomic::write_json(&self.file(SESSION_FILE), project)
    }

    /// «Не сохранять»: drops the working state, the saved project stays.
    pub fn discard_session(&self) -> Result<()> {
        let path = self.file(SESSION_FILE);
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Err(ProjectError::io("удалить", &path)(e))
            }
            _ => Ok(()),
        }
    }

    /// Ctrl+Shift+S: a new folder with copies of `media/` and `assets/` and `project` saved in it.
    pub fn save_as(&self, project: &Project, target: impl Into<PathBuf>) -> Result<Self> {
        check(project)?;
        let target = target.into();
        if target.exists() {
            return Err(ProjectError::AlreadyExists { path: target });
        }
        let copy = Self::create(&target)?;
        copy_tree(&self.media_dir(), &copy.media_dir())?;
        copy_tree(&self.assets_dir(), &copy.assets_dir())?;
        copy.save(project)?;
        Ok(copy)
    }
}

fn check(project: &Project) -> Result<()> {
    let problems = project.problems();
    if problems.is_empty() {
        Ok(())
    } else {
        Err(ProjectError::Inconsistent(problems.join("; ")))
    }
}

pub(crate) fn read_project(path: &Path) -> Result<Project> {
    let corrupt = |message: String| ProjectError::Corrupt {
        path: path.to_path_buf(),
        message,
    };
    let text = std::fs::read_to_string(path).map_err(ProjectError::io("прочитать", path))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| corrupt(e.to_string()))?;
    let value = migrate(value).map_err(|e| match e {
        MigrateError::TooNew { found } => ProjectError::TooNew {
            found,
            supported: dc_core::project::SCHEMA_VERSION,
        },
        MigrateError::MissingVersion => corrupt("нет поля schema_version".to_owned()),
        MigrateError::Failed { from, message } => {
            corrupt(format!("миграция со схемы {from} не удалась: {message}"))
        }
    })?;
    let mut project: Project = serde_json::from_value(value).map_err(|e| corrupt(e.to_string()))?;
    let ids: Vec<_> = project.clips.iter().map(|c| c.id).collect();
    project.clip_ids.ensure_above(ids);
    check(&project)?;
    Ok(project)
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to).map_err(ProjectError::io("создать папку", to))?;
    let entries = std::fs::read_dir(from).map_err(ProjectError::io("прочитать папку", from))?;
    for entry in entries {
        let path = entry
            .map_err(ProjectError::io("прочитать папку", from))?
            .path();
        let Some(name) = path.file_name() else {
            continue;
        };
        if path.is_dir() {
            copy_tree(&path, &to.join(name))?;
        } else {
            std::fs::copy(&path, to.join(name)).map_err(ProjectError::io("скопировать", &path))?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "dir_tests.rs"]
mod tests;
