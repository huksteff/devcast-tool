//! Recognized words of a track, kept in the project's `cache/` next to the path of the track:
//! `media/recording-1/mic.mp4` → `cache/media/recording-1/mic.mp4.<stream>.<model>.<language>.words.json`.
//! Valid while the file keeps its size, time and start; otherwise recognized again.

use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use dc_audio::SourceTrack;
use dc_project::{ProjectDir, atomic};
use serde::{Deserialize, Serialize};

use crate::error::{Result, TranscribeError};
use crate::model::Model;
use crate::recognize::Recognized;

/// What the words were recognized from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Stamp {
    size: u64,
    modified_ns: u128,
    origin: i64,
}

#[derive(Serialize, Deserialize)]
struct Cached {
    stamp: Stamp,
    recognized: Recognized,
}

/// Where the words of `track` by `model` in `language` (`None` = detected) are kept.
pub fn words_cache_path(
    project: &ProjectDir,
    track: &SourceTrack,
    model: Model,
    language: Option<&str>,
) -> PathBuf {
    let relative = track
        .path
        .strip_prefix(project.root())
        .unwrap_or(&track.path);
    let mut path = project.cache_dir();
    path.extend(relative.components().filter_map(|c| match c {
        std::path::Component::Normal(part) => Some(part),
        _ => None,
    }));
    let model = match model {
        Model::Small => "small",
        Model::LargeV3Turbo => "turbo",
    };
    let mut name = path.into_os_string();
    name.push(format!(
        ".{}.{model}.{}.words.json",
        track.stream,
        language.unwrap_or("auto")
    ));
    PathBuf::from(name)
}

fn stamp(track: &SourceTrack) -> Result<Stamp> {
    let metadata =
        std::fs::metadata(&track.path).map_err(TranscribeError::io("прочитать", &track.path))?;
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    Ok(Stamp {
        size: metadata.len(),
        modified_ns,
        origin: track.origin,
    })
}

/// The cached words, if they were recognized from this very file.
pub fn read_words(path: &std::path::Path, track: &SourceTrack) -> Result<Option<Recognized>> {
    let stamp = stamp(track)?;
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    Ok(serde_json::from_str::<Cached>(&text)
        .ok()
        .filter(|c| c.stamp == stamp)
        .map(|c| c.recognized))
}

pub fn write_words(
    path: &std::path::Path,
    track: &SourceTrack,
    recognized: &Recognized,
) -> Result<()> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(TranscribeError::io("создать папку", folder))?;
    }
    let cached = Cached {
        stamp: stamp(track)?,
        recognized: recognized.clone(),
    };
    Ok(atomic::write_json(path, &cached)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::words::Word;

    #[test]
    fn words_come_back_until_the_file_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let project = ProjectDir::create(tmp.path().join("Слова.dcproj")).unwrap();
        let file = project.media_dir().join("mic.mp4");
        std::fs::write(&file, b"sound").unwrap();
        let track = SourceTrack {
            name: "mic".to_owned(),
            path: file.clone(),
            stream: 0,
            origin: 0,
        };
        let path = words_cache_path(&project, &track, Model::Small, Some("ru"));
        assert!(
            path.ends_with("cache/media/mic.mp4.0.small.ru.words.json"),
            "{}",
            path.display()
        );
        assert_eq!(read_words(&path, &track).unwrap(), None);
        let recognized = Recognized {
            language: "ru".to_owned(),
            words: vec![Word {
                text: "привет".to_owned(),
                start: 0,
                end: 500_000,
                probability: 0.9,
            }],
        };
        write_words(&path, &track, &recognized).unwrap();
        assert_eq!(read_words(&path, &track).unwrap(), Some(recognized));
        std::fs::write(&file, b"other sound").unwrap();
        assert_eq!(read_words(&path, &track).unwrap(), None);
    }
}
