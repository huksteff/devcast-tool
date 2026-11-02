//! The settings in memory and their file. The app keeps one [`Store`] for all its windows (a
//! change in one is seen in the others at once); `devcast config` and `devcast hotkeys` load one
//! when the app is not running. Each change is written atomically right away.

use std::path::{Path, PathBuf};

use dc_project::ProjectError;

use crate::model::{AppSettings, FILE};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("файл настроек {path} испорчен ({message}): удали его, DevCast создаст новый")]
    Bad { path: PathBuf, message: String },
    #[error("не удалось записать или прочитать {path}: {source}. Проверь права на папку")]
    File {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Project(#[from] ProjectError),
}

/// The settings file of the app: `%APPDATA%\DevCast\settings.json`.
pub fn file() -> Result<PathBuf, ProjectError> {
    Ok(dc_project::settings::dir()?.join(FILE))
}

#[derive(Debug)]
pub struct Store {
    path: PathBuf,
    settings: AppSettings,
}

impl Store {
    /// Reads `path`; no file yet — the defaults.
    pub fn load(path: &Path) -> Result<Self, StoreError> {
        let settings = match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| StoreError::Bad {
                path: path.to_path_buf(),
                message: e.to_string(),
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppSettings::default(),
            Err(source) => {
                return Err(StoreError::File {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        Ok(Self {
            path: path.to_path_buf(),
            settings,
        })
    }

    pub fn get(&self) -> &AppSettings {
        &self.settings
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Changes the settings with `change` and writes the file if anything changed.
    pub fn update(&mut self, change: impl FnOnce(&mut AppSettings)) -> Result<(), StoreError> {
        let mut next = self.settings.clone();
        change(&mut next);
        if next == self.settings {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| StoreError::File {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        dc_project::atomic::write_json(&self.path, &next)?;
        self.settings = next;
        Ok(())
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
