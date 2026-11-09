use dc_ui_kit::strings::{captions, hud, settings as text};
use egui_kittest::kittest::Queryable;

use super::*;
use crate::downloads::Downloads;
use crate::models::ModelState;
use crate::settings::{Language, Listen, SpeechModel};
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
    let defaults = DefaultsFile::load(dir.path().join("defaults.json")).unwrap();
    let models = Models::new(Some(models), Downloads::default());
    let state = State {
        window: SettingsWindow::new(mics, models, defaults),
        store,
        _dir: dir,
    };
    dc_ui_kit::testing::harness()
        .with_size(SIZE)
        .build_ui_state(
            |ui, state: &mut State| {
                let taken: &[Command] = &[];
                let store = &mut state.store;
                state.window.ui(ui, store, (0.4, taken), None).unwrap();
            },
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
    harness
        .get_by_role_and_label(egui::Role::ComboBox, captions::LANGUAGE)
        .click();
    harness.run();
    harness.get_by_label(captions::RUSSIAN).click();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::ComboBox, captions::TRACKS)
        .click();
    harness.run();
    harness.get_by_label(captions::BOTH).click();
    harness.run();
    let settings = harness.state().store.get();
    assert_eq!(settings.speech_language, Language::Russian);
    assert_eq!(settings.speech_tracks, Listen::Both);
    harness.get_by_label(text::UPDATES).click();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::Switch, text::BETA)
        .click();
    harness.get_by_label(text::CHECK_UPDATES).click();
    harness.run();
    let pressed = harness.state_mut().window.update_action.take();
    assert_eq!(pressed, Some(UpdateAction::Check));
    harness.state_mut().window.update = UpdateState::Downloading {
        version: "0.2.0".to_owned(),
        done: 0.62,
    };
    harness.run();
    harness.get_by_label(&dc_ui_kit::strings::update::downloading("0.2.0", 0.62));
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

#[test]
fn about_page_with_feedback() {
    let mut harness = harness();
    harness.get_by_label(text::ABOUT).click();
    harness.run();
    harness.get_by_label(text::OPEN_LOGS);
    let feedback = dc_update::feedback().unwrap();
    harness.get_by_label(&text::contact(text::REPOSITORY, &feedback.repository));
    harness.snapshot("settings_about");
}

#[test]
fn a_click_on_a_tag_copies_it_and_says_so() {
    let mut harness = harness();
    harness.get_by_label(text::ABOUT).click();
    harness.run();
    harness.get_by_label("@d.novikov").click();
    harness.step();
    let copied = harness
        .output()
        .platform_output
        .commands
        .iter()
        .any(|c| matches!(c, eframe::egui::OutputCommand::CopyText(t) if t == "@d.novikov"));
    assert!(copied, "the tag is on the clipboard");
    harness.run();
    assert!(harness.query_by_label(text::TAG_COPIED).is_some());
}
