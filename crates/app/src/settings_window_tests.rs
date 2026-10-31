use dc_ui_kit::strings::{hud, settings as text};
use egui_kittest::kittest::Queryable;

use super::*;
use crate::models::ModelState;
use crate::settings::SpeechModel;
use crate::settings_captions::Card;

struct State {
    window: SettingsWindow,
    store: Store,
    _dir: tempfile::TempDir,
}

fn harness() -> egui_kittest::Harness<'static, State> {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::load(&dir.path().join("settings.json")).unwrap();
    // The default folder holds the user's name: the snapshot would differ from PC to PC.
    store
        .update(|s| s.recordings_dir = std::path::PathBuf::from(r"D:\Записи\DevCast"))
        .unwrap();
    let models = dir.path().join("models");
    std::fs::create_dir(&models).unwrap();
    let fast = SpeechModel::Fast.model().file_name();
    std::fs::write(models.join(fast), b"model").unwrap();
    let mics = vec![
        "Микрофон (Rode NT-USB)".to_owned(),
        "Гарнитура (HyperX Cloud II)".to_owned(),
    ];
    let state = State {
        window: SettingsWindow::new(mics, Models::new(Some(models))),
        store,
        _dir: dir,
    };
    dc_ui_kit::testing::harness()
        .with_size(SIZE)
        .build_ui_state(
            |ui, state: &mut State| state.window.ui(ui, &mut state.store, 0.4, &[]).unwrap(),
            state,
        )
}

#[test]
fn recording_page_saves_at_once() {
    let mut harness = harness();
    harness.run();
    harness.snapshot("settings_recording");
    harness.get_by_label("120").click();
    harness.run();
    harness.get_by_label("10 с").click();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::Switch, hud::SYSTEM_AUDIO)
        .click();
    harness.run();
    let settings = harness.state().store.get().clone();
    assert_eq!(settings.recording.fps, Fps::Fps120);
    assert_eq!(settings.countdown, Countdown::Ten);
    assert!(!settings.recording.system_audio);
    let path = harness.state()._dir.path().join("settings.json");
    let saved = Store::load(&path).unwrap();
    assert_eq!(
        saved.get(),
        &settings,
        "every change is in the file at once"
    );
}

#[test]
fn captions_and_updates_pages() {
    let mut harness = harness();
    harness.get_by_label(text::CAPTIONS).click();
    harness.run();
    harness.snapshot("settings_captions");
    harness.get_by_label(text::MODEL_ACCURATE).click();
    harness.run();
    assert_eq!(
        harness.state().store.get().speech_model,
        SpeechModel::Accurate
    );
    harness.get_by_label(text::UPDATES).click();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::Switch, text::BETA)
        .click();
    harness.get_by_label(text::CHECK_UPDATES).click();
    harness.run();
    harness.get_by_label(text::MANUAL_UPDATES);
    assert!(harness.state().store.get().updates.beta);
    harness.snapshot("settings_updates");
}

#[test]
fn model_card_while_downloading() {
    let card = Card {
        title: text::MODEL_ACCURATE,
        hint: text::MODEL_ACCURATE_HINT,
        chosen: false,
        state: ModelState::Downloading(Some(0.42)),
        can_download: true,
    };
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(260.0, 100.0))
        .build_ui(move |ui| {
            let height = card.height(ui, 240.0);
            card.show(ui, egui::vec2(240.0, height));
        });
    harness.run();
    harness.snapshot("settings_model_downloading");
}
