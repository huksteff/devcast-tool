use dc_capture::{Area, Fps, Source};

use super::*;
use crate::model::{Countdown, SpeechModel, Updates};

fn path_in(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("Настройки DevCast").join(FILE)
}

#[test]
fn defaults_when_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::load(&path_in(&dir)).unwrap();
    let settings = store.get();
    assert_eq!(settings.recording.source, Source::Monitor(1));
    assert_eq!(settings.recording.fps, Fps::Fps60);
    assert_eq!(settings.countdown.seconds(), 3);
    assert!(settings.hide_hud);
    assert_eq!(settings.speech_model, SpeechModel::Fast);
    assert_eq!(settings.hotkeys.start_stop.as_deref(), Some("Ctrl+Shift+R"));
    assert!(settings.recordings_dir.ends_with("DevCast"));
    assert!(!path_in(&dir).exists(), "loading writes nothing");
}

#[test]
fn a_change_is_saved_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = path_in(&dir);
    let mut store = Store::load(&path).unwrap();
    store
        .update(|s| {
            s.recordings_dir = dir.path().join("Мои записи с пробелом");
            s.recording.source = Source::Area(Area::new(-1700, 150, 1280, 720).unwrap());
            s.recording.fps = Fps::Fps120;
            s.recording.microphone = Some("Микрофон (Rode NT-USB)".to_owned());
            s.countdown = Countdown::Ten;
            s.hotkeys.pause = None;
            s.speech_model = SpeechModel::Accurate;
            s.updates.beta = true;
        })
        .unwrap();
    let again = Store::load(&path).unwrap();
    assert_eq!(again.get(), store.get());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"countdown\": 10"), "{text}");
    assert!(
        text.contains("\"source\": \"area:-1700,150,1280,720\""),
        "{text}"
    );
}

#[test]
fn no_change_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = path_in(&dir);
    let mut store = Store::load(&path).unwrap();
    store.update(|s| s.hide_hud = true).unwrap();
    assert!(!path.exists());
}

#[test]
fn missing_fields_take_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = path_in(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, r#"{"countdown": 5}"#).unwrap();
    let store = Store::load(&path).unwrap();
    assert_eq!(store.get().countdown, Countdown::Five);
    assert_eq!(store.get().updates, Updates::default());
}

#[test]
fn bad_file_says_what_to_do() {
    let dir = tempfile::tempdir().unwrap();
    let path = path_in(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, r#"{"countdown": 4}"#).unwrap();
    let error = Store::load(&path).unwrap_err().to_string();
    assert!(error.contains("0, 3, 5 или 10"), "{error}");
    assert!(error.contains("удали"), "{error}");
}
