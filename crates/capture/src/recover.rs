//! Recovering a recording whose process died: keep the segments the manifest lists, drop the
//! unfinished last one, and finalize as after a normal stop.

use std::path::{Path, PathBuf};

use crate::error::CaptureError;
use crate::finalize::finalize;
use crate::manifest::Manifest;

/// Builds each track's file from the complete segments of `dir` and deletes the rest. Safe to
/// run again if it is interrupted, and on an already finished recording (then it only removes
/// leftovers). Returns the path of each track's file.
pub fn recover(dir: &Path) -> Result<Vec<PathBuf>, CaptureError> {
    let mut manifest = Manifest::read(dir)?;
    finalize(dir, &mut manifest)
}
