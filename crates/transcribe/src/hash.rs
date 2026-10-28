//! SHA-256 of model files, the same check for fresh downloads and files already on disk.

use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{Result, TranscribeError};

/// Lowercase hex SHA-256 of the file, read in chunks: models are up to 1.6 GB.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(TranscribeError::io("открыть", path))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 1 << 20];
    loop {
        let read = file
            .read(&mut buf)
            .map_err(TranscribeError::io("прочитать", path))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
