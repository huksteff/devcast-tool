use dc_capture::{Area, MonitorInfo, Source, SourceList, WindowInfo};
use dc_ui_kit::strings::hud;
use dc_ui_kit::tokens::SURFACE;
use eframe::egui;
use egui_kittest::kittest::Queryable;

use super::bar::{self, Clicked};
use super::popover::{self, MoreAction};
use crate::settings::{AppSettings, Countdown};

fn sources() -> SourceList {
    SourceList {
        monitors: vec![MonitorInfo {
            index: 1,
            name: None,
            bounds: Area::new(0, 0, 2560, 1440).unwrap(),
            primary: true,
        }],
        windows: vec![WindowInfo {
            id: 42,
            title: "main.rs — Visual Studio Code".to_owned(),
            app: Some("Code".to_owned()),
            minimized: false,
        }],
    }
}

#[test]
fn source_names() {
    let list = sources();
    assert_eq!(
        bar::source_label(&Source::Monitor(1), &list),
        "Экран 1 · 2560×1440"
    );
    assert_eq!(bar::source_label(&Source::Monitor(3), &list), "Экран 3");
    assert_eq!(
        bar::source_label(&Source::Window(42), &list),
        "main.rs — Visual Studio Code"
    );
    assert_eq!(bar::source_label(&Source::Window(7), &list), "Окно");
    let area = Source::Area(Area::new(10, 10, 1280, 720).unwrap());
    assert_eq!(bar::source_label(&area, &list), "Область · 1280×720");
}

struct Bar {
    settings: AppSettings,
    record: bar::RecordButton,
    clicked: Option<Clicked>,
}

fn bar_harness(settings: AppSettings) -> egui_kittest::Harness<'static, Bar> {
    record_harness(settings, bar::RecordButton::Ready)
}

fn record_harness(
    settings: AppSettings,
    record: bar::RecordButton,
) -> egui_kittest::Harness<'static, Bar> {
    let label = bar::source_label(&settings.recording.source, &sources());
    dc_ui_kit::testing::harness()
        .with_size(egui::vec2(500.0, 48.0))
        .build_ui_state(
            move |ui, state: &mut Bar| {
                egui::Frame::new().fill(SURFACE).show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    let inner = ui.horizontal_centered(|ui| {
                        bar::bar(ui, &state.settings, &label, state.record)
                    });
                    if let Some((clicked, _)) = inner.inner {
                        state.clicked = Some(clicked);
                    }
                });
            },
            Bar {
                settings,
                record,
                clicked: None,
            },
        )
}

#[test]
fn bar_at_rest() {
    let defaults = AppSettings::default();
    let settings = AppSettings {
        recording: dc_capture::RecordingConfig {
            microphone: Some("Микрофон (Rode NT-USB)".to_owned()),
            ..defaults.recording.clone()
        },
        ..defaults
    };
    let mut harness = bar_harness(settings);
    harness.run();
    harness.snapshot("hud_idle");
    harness.get_by_label(hud::MORE).click();
    harness.run();
    assert_eq!(harness.state().clicked, Some(Clicked::More));
    harness.get_by_label(hud::RECORD).click();
    harness.run();
    assert_eq!(harness.state().clicked, Some(Clicked::Record));
}

#[test]
fn bar_without_mic_and_countdown() {
    let defaults = AppSettings::default();
    let settings = AppSettings {
        countdown: Countdown::Off,
        recording: dc_capture::RecordingConfig {
            system_audio: false,
            ..defaults.recording.clone()
        },
        ..defaults
    };
    let mut harness = bar_harness(settings);
    harness.run();
    harness.snapshot("hud_idle_off");
}

#[test]
fn more_menu() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(280.0, 190.0))
        .build_ui_state(
            |ui, state: &mut (bool, Option<MoreAction>)| {
                dc_ui_kit::widgets::panel_frame().show(ui, |ui| {
                    if let Some(action) = popover::more_menu(ui, &mut state.0) {
                        state.1 = Some(action);
                    }
                });
            },
            (true, None),
        );
    harness.run();
    harness.snapshot("hud_more");
    harness
        .get_by_role_and_label(egui::Role::Switch, hud::HIDE_FROM_RECORDING)
        .click();
    harness.get_by_label(hud::SETTINGS).click();
    harness.run();
    assert_eq!(*harness.state(), (false, Some(MoreAction::Settings)));
}

#[test]
fn source_picker() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(super::picker::WIDTH, 420.0))
        .build_ui_state(
            |ui, chosen: &mut Source| {
                dc_ui_kit::widgets::panel_frame().show(ui, |ui| {
                    ui.set_width(super::picker::WIDTH - 26.0);
                    let textures = std::collections::HashMap::new();
                    let last = Some((1280, 720));
                    let picked = super::picker::picker(ui, &sources(), chosen, &textures, last);
                    if let Some(super::picker::Pick::Source(source)) = picked {
                        *chosen = source;
                    }
                });
            },
            Source::Monitor(1),
        );
    harness.run();
    harness.snapshot("hud_picker");
    harness.get_by_label("main.rs — Visual Studio Code").click();
    harness.run();
    assert_eq!(*harness.state(), Source::Window(42));
}

#[test]
fn sound_menu_picks_a_microphone() {
    let mics = vec![
        "Микрофон (Rode NT-USB)".to_owned(),
        "Гарнитура (HyperX Cloud II)".to_owned(),
    ];
    let start = dc_capture::RecordingConfig {
        microphone: Some(mics[0].clone()),
        ..AppSettings::default().recording
    };
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(super::menus::SOUND_WIDTH, 300.0))
        .build_ui_state(
            move |ui, recording: &mut dc_capture::RecordingConfig| {
                dc_ui_kit::widgets::panel_frame().show(ui, |ui| {
                    ui.set_width(super::menus::SOUND_WIDTH - 26.0);
                    let level = |source: &dc_capture::audio::AudioSource| match source {
                        dc_capture::audio::AudioSource::System => 0.38,
                        dc_capture::audio::AudioSource::Microphone(name)
                            if name.starts_with("Мик") =>
                        {
                            0.78
                        }
                        _ => 0.12,
                    };
                    super::menus::sound_menu(ui, recording, &mics, &level);
                });
            },
            start,
        );
    harness.run();
    harness.snapshot("hud_sound");
    harness.get_by_label("Гарнитура (HyperX Cloud II)").click();
    harness.run();
    assert_eq!(
        harness.state().microphone.as_deref(),
        Some("Гарнитура (HyperX Cloud II)")
    );
    harness.get_by_label(hud::NO_MIC).click();
    harness
        .get_by_role_and_label(egui::Role::Switch, hud::SYSTEM_AUDIO)
        .click();
    harness.run();
    assert_eq!(harness.state().microphone, None);
    assert!(!harness.state().system_audio);
}

#[test]
fn timing_menu_sets_fps_and_countdown() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(super::menus::TIMING_WIDTH, 230.0))
        .build_ui_state(
            |ui, settings: &mut AppSettings| {
                dc_ui_kit::widgets::panel_frame().show(ui, |ui| {
                    super::menus::timing_menu(ui, settings);
                });
            },
            AppSettings::default(),
        );
    harness.run();
    harness.snapshot("hud_timing");
    harness.get_by_label("120").click();
    harness.run();
    harness.get_by_label("Нет").click();
    harness.run();
    assert_eq!(harness.state().recording.fps, dc_capture::Fps::Fps120);
    assert_eq!(harness.state().countdown, Countdown::Off);
}

#[test]
fn countdown_turns_record_into_cancel() {
    let mut harness = record_harness(AppSettings::default(), bar::RecordButton::Countdown(3));
    harness.run();
    harness.snapshot("hud_countdown");
    harness.get_by_label("Отменить 3").click();
    harness.run();
    assert_eq!(harness.state().clicked, Some(Clicked::Record));
}

fn live_harness(view: super::live::LiveView) -> egui_kittest::Harness<'static, Option<Clicked>> {
    dc_ui_kit::testing::harness()
        .with_size(egui::vec2(380.0, 48.0))
        .build_ui_state(
            move |ui, clicked: &mut Option<Clicked>| {
                let border = if view.paused {
                    dc_ui_kit::tokens::PAUSE
                } else {
                    dc_ui_kit::tokens::BORDER
                };
                ui.painter().rect(
                    ui.max_rect(),
                    dc_ui_kit::tokens::R3,
                    SURFACE,
                    egui::Stroke::new(1.0, border),
                    egui::StrokeKind::Inside,
                );
                let inner = ui.horizontal_centered(|ui| super::live::live_bar(ui, &view));
                if let Some((c, _)) = inner.inner {
                    *clicked = Some(c);
                }
            },
            None,
        )
}

#[test]
fn recording_bar() {
    let mut harness = live_harness(super::live::LiveView {
        timer: "12:47".to_owned(),
        paused: false,
        mic: Some(0.6),
    });
    harness.run();
    harness.snapshot("hud_recording");
    for (label, clicked) in [
        (hud::STOP, Clicked::Stop),
        (hud::PAUSE, Clicked::Pause),
        (hud::CANCEL, Clicked::Cancel),
        (hud::HIDE, Clicked::Hide),
    ] {
        harness.get_by_label(label).click();
        harness.run();
        assert_eq!(*harness.state(), Some(clicked));
    }
}

#[test]
fn paused_bar() {
    let mut harness = live_harness(super::live::LiveView {
        timer: "12:47".to_owned(),
        paused: true,
        mic: None,
    });
    harness.run();
    harness.snapshot("hud_paused");
    harness.get_by_label(hud::RESUME).click();
    harness.run();
    assert_eq!(*harness.state(), Some(Clicked::Pause));
}

#[test]
fn cancel_question() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(280.0, 150.0))
        .build_ui_state(
            |ui, answer: &mut Option<dc_ui_kit::widgets::Answer>| {
                dc_ui_kit::widgets::panel_frame().show(ui, |ui| {
                    let text = hud::cancel_text("12:47");
                    let question = dc_ui_kit::widgets::Confirm {
                        title: hud::CANCEL_TITLE,
                        text: &text,
                        cancel: hud::KEEP_RECORDING,
                        confirm: hud::DELETE_RECORDING,
                        kind: dc_ui_kit::widgets::Kind::Danger,
                    };
                    if let Some(a) = question.show(ui) {
                        *answer = Some(a);
                    }
                });
            },
            None,
        );
    harness.run();
    harness.snapshot("hud_cancel");
    harness.get_by_label(hud::DELETE_RECORDING).click();
    harness.run();
    assert_eq!(*harness.state(), Some(dc_ui_kit::widgets::Answer::Confirm));
}

#[test]
fn preparing_bar() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(super::live::PREPARING_WIDTH, 48.0))
        .build_ui(|ui| {
            ui.painter().rect_filled(ui.max_rect(), 0, SURFACE);
            ui.horizontal_centered(super::live::preparing);
        });
    harness.run();
    harness.snapshot("hud_preparing");
}

#[test]
fn countdown_window() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(crate::countdown::SIDE, crate::countdown::SIDE))
        .build_ui_state(
            |ui, cancelled: &mut bool| {
                if crate::countdown::face(ui, 3) {
                    *cancelled = true;
                }
            },
            false,
        );
    harness.run();
    harness.snapshot("countdown");
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(*harness.state());
}
