//! Whisper GGML models: where they come from, their hashes and where they are stored
//! (docs/decisions/010-whisper.md).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;

const SOURCE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Model {
    /// «Быстрая»: 488 MB, usable on CPU.
    Small,
    /// «Точная»: 1.62 GB, needs a GPU.
    LargeV3Turbo,
}

impl Model {
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Small => "ggml-small.bin",
            Self::LargeV3Turbo => "ggml-large-v3-turbo.bin",
        }
    }

    pub fn url(self) -> String {
        format!("{SOURCE}{}", self.file_name())
    }

    /// Approximate size of the download, MB (for the lists; the hash checks the file).
    pub const fn megabytes(self) -> u32 {
        match self {
            Self::Small => 488,
            Self::LargeV3Turbo => 1624,
        }
    }

    /// Lowercase hex SHA-256 of the model file.
    pub fn sha256(self) -> &'static str {
        match self {
            Self::Small => "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
            Self::LargeV3Turbo => {
                "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69"
            }
        }
    }
}

/// The folder of the models in the app settings folder.
pub const FOLDER: &str = "models";

/// Models live in the app settings folder: `%APPDATA%\DevCast\models`.
pub fn models_dir() -> Result<PathBuf> {
    Ok(dc_project::settings::dir()?.join(FOLDER))
}
