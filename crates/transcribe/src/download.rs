//! Model download: resumable (HTTP Range) into `<file>.part`, SHA-256 check, then rename.
//! A file under the final name is always a verified one.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{Result, TranscribeError};
use crate::hash::sha256_file;
use crate::model::Model;

/// Downloaded bytes of the whole file, including the part fetched before a resume.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Progress {
    pub done: u64,
    /// `None` if the server did not say the size.
    pub total: Option<u64>,
}

/// Path of the model in `dir`, downloading it first if it is not there yet.
pub fn ensure(
    model: Model,
    dir: &Path,
    on_progress: &mut dyn FnMut(Progress),
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    let path = dir.join(model.file_name());
    if path
        .try_exists()
        .map_err(TranscribeError::io("проверить", &path))?
    {
        return Ok(path);
    }
    std::fs::create_dir_all(dir).map_err(TranscribeError::io("создать папку", dir))?;
    download(&model.url(), model.sha256(), &path, on_progress, cancel)?;
    Ok(path)
}

/// Downloads `url` to `dest`. An interrupted or cancelled download keeps `<dest>.part` and
/// continues from it next time; a part with the wrong hash is deleted.
pub fn download(
    url: &str,
    sha256: &str,
    dest: &Path,
    on_progress: &mut dyn FnMut(Progress),
    cancel: &AtomicBool,
) -> Result<()> {
    let part = part_path(dest);
    fetch(url, &part, on_progress, cancel)?;
    let actual = sha256_file(&part)?;
    if actual != sha256 {
        std::fs::remove_file(&part).map_err(TranscribeError::io("удалить", &part))?;
        return Err(TranscribeError::HashMismatch {
            path: dest.to_path_buf(),
            expected: sha256.to_owned(),
            actual,
        });
    }
    std::fs::rename(&part, dest).map_err(TranscribeError::io("переименовать", &part))
}

fn part_path(dest: &Path) -> PathBuf {
    let mut part = dest.as_os_str().to_owned();
    part.push(".part");
    PathBuf::from(part)
}

/// Appends the rest of the file to `part`. 416 means the part already holds the whole file:
/// the hash check decides whether it is the right one.
fn fetch(
    url: &str,
    part: &Path,
    on_progress: &mut dyn FnMut(Progress),
    cancel: &AtomicBool,
) -> Result<()> {
    let offset = match std::fs::metadata(part) {
        Ok(meta) => meta.len(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(e) => return Err(TranscribeError::io("проверить", part)(e)),
    };
    let agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .new_agent();
    let mut request = agent.get(url);
    if offset > 0 {
        request = request.header("Range", format!("bytes={offset}-"));
    }
    let response = request.call().map_err(|e| network(url, e.into_io()))?;
    let length = response.body().content_length();
    let (start, total) = match response.status().as_u16() {
        200 => (0, length),
        206 => {
            let content_range = response
                .headers()
                .get("Content-Range")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default();
            match parse_content_range(content_range) {
                Some((start, total)) if start == offset => (start, total),
                _ => {
                    return Err(TranscribeError::BadRange {
                        url: url.to_owned(),
                        expected: offset,
                        content_range: content_range.to_owned(),
                        part: part.to_path_buf(),
                    });
                }
            }
        }
        416 => return Ok(()),
        status => {
            return Err(TranscribeError::Status {
                url: url.to_owned(),
                status,
            });
        }
    };
    let mut file = if start == 0 {
        File::create(part)
    } else {
        OpenOptions::new().append(true).open(part)
    }
    .map_err(TranscribeError::io("открыть", part))?;
    let mut body = response.into_body().into_reader();
    copy(&mut body, &mut file, start, total, on_progress, cancel)
        .map_err(|e| e.with_paths(url, part))?;
    file.sync_all()
        .map_err(TranscribeError::io("сбросить на диск", part))
}

enum CopyError {
    Read(std::io::Error),
    Write(std::io::Error),
    Cancelled,
}

impl CopyError {
    fn with_paths(self, url: &str, part: &Path) -> TranscribeError {
        match self {
            Self::Read(source) => network(url, source),
            Self::Write(source) => TranscribeError::io("записать", part)(source),
            Self::Cancelled => TranscribeError::Cancelled,
        }
    }
}

fn copy(
    body: &mut impl Read,
    file: &mut File,
    start: u64,
    total: Option<u64>,
    on_progress: &mut dyn FnMut(Progress),
    cancel: &AtomicBool,
) -> std::result::Result<(), CopyError> {
    let mut buf = vec![0; 1 << 16];
    let mut done = start;
    on_progress(Progress { done, total });
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(CopyError::Cancelled);
        }
        let read = body.read(&mut buf).map_err(CopyError::Read)?;
        if read == 0 {
            return Ok(());
        }
        file.write_all(&buf[..read]).map_err(CopyError::Write)?;
        done += read as u64;
        on_progress(Progress { done, total });
    }
}

/// `bytes <start>-<end>/<total|*>` → start and total.
fn parse_content_range(value: &str) -> Option<(u64, Option<u64>)> {
    let (range, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let start = range.split_once('-')?.0.parse().ok()?;
    Some((start, total.parse().ok()))
}

fn network(url: &str, source: std::io::Error) -> TranscribeError {
    TranscribeError::Network {
        url: url.to_owned(),
        source,
    }
}

#[cfg(test)]
#[path = "download_tests.rs"]
mod tests;
