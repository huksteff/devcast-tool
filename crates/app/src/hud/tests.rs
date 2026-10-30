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
    clicked: Option<Clicked>,
}

fn bar_harness(settings: AppSettings) -> egui_kittest::Harness<'static, Bar> {
    let label = bar::source_label(&settings.recording.source, &sources());
    dc_ui_kit::testing::harness()
        .with_size(egui::vec2(500.0, 48.0))
        .build_ui_state(
            move |ui, state: &mut Bar| {
                egui::Frame::new().fill(SURFACE).show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    let inner = ui.horizontal_centered(|ui| bar::bar(ui, &state.settings, &label));
                    if let Some((clicked, _)) = inner.inner {
                        state.clicked = Some(clicked);
                    }
                });
            },
            Bar {
                settings,
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
