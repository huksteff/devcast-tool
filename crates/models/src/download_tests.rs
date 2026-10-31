use std::sync::atomic::{AtomicBool, Ordering};

use sha2::{Digest, Sha256};

use super::*;
use crate::test_server::Server;

const SIZE: usize = 300_000;

fn data() -> Vec<u8> {
    (0..SIZE).map(|i| (i * 31 % 251) as u8).collect()
}

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn dest() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix("модели whisper ")
        .tempdir()
        .unwrap();
    let dest = dir.path().join("ggml-small.bin");
    (dir, dest)
}

fn go(server: &Server, sha256: &str, dest: &Path, cancel: &AtomicBool) -> Result<Vec<Progress>> {
    let mut events = Vec::new();
    download(&server.url, sha256, dest, &mut |p| events.push(p), cancel)?;
    Ok(events)
}

#[test]
fn downloads_verifies_and_renames() {
    let data = data();
    let server = Server::start(data.clone(), None);
    let (_dir, dest) = dest();
    let events = go(&server, &sha(&data), &dest, &AtomicBool::new(false)).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    assert!(!part_path(&dest).exists());
    let total = Some(SIZE as u64);
    assert_eq!(events.first(), Some(&Progress { done: 0, total }));
    assert_eq!(
        events.last(),
        Some(&Progress {
            done: SIZE as u64,
            total
        })
    );
    assert!(events.windows(2).all(|w| w[0].done <= w[1].done));
}

#[test]
fn resumes_after_dropped_connection() {
    let data = data();
    let server = Server::start(data.clone(), Some(100_000));
    let (_dir, dest) = dest();
    let cancel = AtomicBool::new(false);
    let err = go(&server, &sha(&data), &dest, &cancel).unwrap_err();
    assert!(matches!(err, ModelsError::Network { .. }), "{err}");
    assert!(!dest.exists());
    let kept = std::fs::metadata(part_path(&dest)).unwrap().len();
    assert!(kept > 0 && kept <= 100_000, "{kept}");

    let events = go(&server, &sha(&data), &dest, &cancel).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    assert_eq!(*server.requests.lock().unwrap(), vec![0, kept]);
    let total = Some(SIZE as u64);
    assert_eq!(events.first(), Some(&Progress { done: kept, total }));
}

#[test]
fn wrong_hash_deletes_part_and_starts_over() {
    let data = data();
    let server = Server::start(data.clone(), None);
    let (_dir, dest) = dest();
    let cancel = AtomicBool::new(false);
    let err = go(&server, &sha(b"other file"), &dest, &cancel).unwrap_err();
    assert!(matches!(err, ModelsError::HashMismatch { .. }), "{err}");
    assert!(!dest.exists());
    assert!(!part_path(&dest).exists());

    go(&server, &sha(&data), &dest, &cancel).unwrap();
    assert_eq!(*server.requests.lock().unwrap(), vec![0, 0]);
}

#[test]
fn foreign_part_is_rejected_by_hash() {
    let data = data();
    let server = Server::start(data.clone(), None);
    let (_dir, dest) = dest();
    std::fs::write(part_path(&dest), vec![7; SIZE]).unwrap();
    let cancel = AtomicBool::new(false);
    let err = go(&server, &sha(&data), &dest, &cancel).unwrap_err();
    assert!(matches!(err, ModelsError::HashMismatch { .. }), "{err}");
    assert!(!part_path(&dest).exists());

    go(&server, &sha(&data), &dest, &cancel).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), data);
}

#[test]
fn complete_part_is_verified_without_body() {
    let data = data();
    let server = Server::start(data.clone(), None);
    let (_dir, dest) = dest();
    std::fs::write(part_path(&dest), &data).unwrap();
    let events = go(&server, &sha(&data), &dest, &AtomicBool::new(false)).unwrap();
    assert!(events.is_empty());
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    assert_eq!(*server.requests.lock().unwrap(), vec![SIZE as u64]);
}

#[test]
fn cancel_keeps_part_for_resume() {
    let data = data();
    let server = Server::start(data.clone(), None);
    let (_dir, dest) = dest();
    let cancel = AtomicBool::new(false);
    let mut on_progress = |p: Progress| {
        if p.done > 0 {
            cancel.store(true, Ordering::Relaxed);
        }
    };
    let err = download(&server.url, &sha(&data), &dest, &mut on_progress, &cancel).unwrap_err();
    assert!(matches!(err, ModelsError::Cancelled), "{err}");
    assert!(!dest.exists());
    let kept = std::fs::metadata(part_path(&dest)).unwrap().len();
    assert!(kept > 0 && kept < SIZE as u64, "{kept}");

    cancel.store(false, Ordering::Relaxed);
    go(&server, &sha(&data), &dest, &cancel).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), data);
}

#[test]
fn ensure_returns_existing_model_without_network() {
    let (dir, dest) = dest();
    std::fs::write(&dest, b"model").unwrap();
    let path = ensure(
        Model::Small,
        dir.path(),
        &mut |_| panic!("no download expected"),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(path, dest);
}

#[test]
#[ignore = "needs the real models in .deps/models (2.1 GB of hashing)"]
fn real_models_match_their_hashes() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.deps/models");
    for model in [Model::Small, Model::LargeV3Turbo] {
        let actual = sha256_file(&dir.join(model.file_name())).unwrap();
        assert_eq!(actual, model.sha256(), "{model:?}");
    }
}
