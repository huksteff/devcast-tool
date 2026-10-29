//! Helpers of the unit tests: files made by the FFmpeg command line from `.deps`, fixtures.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::track::SourceTrack;

/// Runs the FFmpeg command line from `.deps` with `args` after `-v error -y`.
pub(crate) fn ffmpeg(args: &[&str]) {
    let tool = PathBuf::from(env!("FFMPEG_DIR"))
        .join("bin")
        .join("ffmpeg.exe");
    let status = Command::new(tool)
        .args(["-v", "error", "-y"])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg {args:?}");
}

/// `path` inside `fixtures/`.
pub(crate) fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(path)
}

/// Stream 0 of `path`, read from the start of the file's clock.
pub(crate) fn track(path: &Path) -> SourceTrack {
    SourceTrack {
        name: "test".to_owned(),
        path: path.to_path_buf(),
        stream: 0,
        origin: 0,
    }
}
