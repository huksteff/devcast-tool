//! The shared downloads without the network: a download is "running" once it is registered
//! with no thread, and the tests play the thread's part (`progress`, `finish`).

use std::path::PathBuf;
use std::sync::mpsc;

use dc_models::ModelsError;

use super::*;
use crate::models::{ModelState, Models};

const MODEL: SpeechModel = SpeechModel::Accurate;

/// A download of [`MODEL`] registered by a window `u64::MAX` and no thread; its news go to the
/// returned receiver.
fn running(downloads: &Downloads) -> mpsc::Receiver<News> {
    let (news, receiver) = mpsc::channel();
    let listener = Listener {
        owner: u64::MAX,
        news,
        wake: Box::new(|| {}),
    };
    let joined = downloads.join_or(MODEL, listener, |_| Ok(())).unwrap();
    assert_eq!(joined, Joined::Started);
    receiver
}

fn window(downloads: &Downloads) -> Models {
    let mut models = Models::new(Some(PathBuf::from("unused")), downloads.clone());
    models.download(|| {}, MODEL).unwrap();
    models
}

#[test]
fn a_second_window_joins_the_running_download() {
    let downloads = Downloads::default();
    let _first = running(&downloads);
    downloads.progress(MODEL, 0.25);
    let mut settings = window(&downloads);
    let mut editor = window(&downloads);
    assert_eq!(settings.state(MODEL), ModelState::Downloading(Some(0.25)));
    downloads.progress(MODEL, 0.5);
    for models in [&mut settings, &mut editor] {
        models.poll().unwrap();
        assert_eq!(models.state(MODEL), ModelState::Downloading(Some(0.5)));
    }
    assert!(downloads.finish(MODEL, Ok(())));
    assert!(!downloads.is_running(MODEL));
    for models in [&mut settings, &mut editor] {
        models.poll().unwrap();
        assert_eq!(
            models.state(MODEL),
            ModelState::Ready,
            "both learn it is done"
        );
    }
}

#[test]
fn a_failure_reaches_every_window_once() {
    let downloads = Downloads::default();
    let _first = running(&downloads);
    let mut settings = window(&downloads);
    let mut editor = window(&downloads);
    let failure = ModelsError::Status {
        url: "http://127.0.0.1/model.bin".to_owned(),
        status: 503,
    };
    assert!(downloads.finish(MODEL, Err(failure)));
    for models in [&mut settings, &mut editor] {
        let error = models.poll().unwrap_err();
        assert!(error.to_string().contains("503"), "{error}");
        models.poll().unwrap();
        assert_eq!(models.state(MODEL), ModelState::Missing);
    }
}

#[test]
fn the_last_window_to_leave_cancels() {
    let downloads = Downloads::default();
    let _first = running(&downloads);
    let cancel = downloads.cancel_flag(MODEL);
    let settings = window(&downloads);
    let editor = window(&downloads);
    downloads.leave(MODEL, u64::MAX);
    drop(settings);
    assert!(!cancel.load(Ordering::Relaxed), "the editor still waits");
    drop(editor);
    assert!(cancel.load(Ordering::Relaxed));
    assert!(downloads.finish(MODEL, Err(ModelsError::Cancelled)));
    assert!(!downloads.is_running(MODEL));
}

#[test]
fn a_window_joining_a_cancelled_download_resumes_it() {
    let downloads = Downloads::default();
    let receiver = running(&downloads);
    let cancel = downloads.cancel_flag(MODEL);
    downloads.leave(MODEL, u64::MAX);
    assert!(cancel.load(Ordering::Relaxed));
    let mut editor = window(&downloads);
    assert!(
        !cancel.load(Ordering::Relaxed),
        "joining takes the cancel back"
    );
    assert!(
        !downloads.finish(MODEL, Err(ModelsError::Cancelled)),
        "the thread saw the cancel too late: it downloads again"
    );
    assert!(downloads.is_running(MODEL));
    assert!(downloads.finish(MODEL, Ok(())));
    editor.poll().unwrap();
    assert_eq!(editor.state(MODEL), ModelState::Ready);
    assert!(
        receiver.try_recv().is_err(),
        "the window that left hears nothing"
    );
}
