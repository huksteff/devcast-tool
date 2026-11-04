use dc_capture::{Area, MonitorInfo, Source, SourceList, WindowInfo};
use dc_ui_kit::strings::hud;
use dc_ui_kit::tokens::SURFACE;
use eframe::egui;
use egui_kittest::kittest::Queryable;

use super::bar::{self, Clicked};
use super::menus::{self, MoreAction};
use super::popover;
use crate::settings::{AppSettings, Countdown};

fn sources() -> SourceList {
    SourceList {
        monitors: vec![MonitorInfo {
            index: 1,
            name: None,
            bounds: Area::new(0, 0, 2560, 1440).unwrap(),
            primary: true,
            scale: 150,
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
                    if let Some(action) = menus::more_menu(ui, &mut state.0) {
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

/// Two screens and the windows of a usual desktop, with real titles.
fn desktop() -> SourceList {
    let titles = [
        "main.rs — Visual Studio Code",
        "DevCast v2 — окна записи (этап 6) — Google Chrome",
        "Telegram (12)",
        r"Проводник — D:\Kefir\recordy-fork",
        "Unity 6000.0.23f1 — SampleScene — Android — DX11",
        "Slack | #rendering | Kefir Games",
        "Диспетчер задач",
    ];
    SourceList {
        monitors: vec![
            MonitorInfo {
                index: 1,
                name: None,
                bounds: Area::new(0, 0, 1920, 1080).unwrap(),
                primary: true,
                scale: 100,
            },
            MonitorInfo {
                index: 2,
                name: None,
                bounds: Area::new(-1920, 11, 1920, 1080).unwrap(),
                primary: false,
                scale: 100,
            },
        ],
        windows: titles
            .iter()
            .enumerate()
            .map(|(i, title)| WindowInfo {
                id: 100 + i as u64,
                title: (*title).to_owned(),
                app: None,
                minimized: false,
            })
            .collect(),
    }
}

#[test]
fn source_picker_with_a_real_desktop() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(super::picker::WIDTH, super::popover::MAX_HEIGHT))
        .build_ui_state(
            |ui, chosen: &mut Source| {
                dc_ui_kit::widgets::panel_frame()
                    .inner_margin(popover::Kind::Source.padding())
                    .show(ui, |ui| {
                        ui.set_width(super::picker::WIDTH - 34.0);
                        let textures = std::collections::HashMap::new();
                        let picked = super::picker::picker(ui, &desktop(), chosen, &textures, None);
                        if let Some(super::picker::Pick::Source(source)) = picked {
                            *chosen = source;
                        }
                    });
            },
            Source::Monitor(1),
        );
    harness.run();
    harness.snapshot("hud_picker_desktop");
    harness.get_by_label("Диспетчер задач").click();
    harness.run();
    assert_eq!(*harness.state(), Source::Window(106));
}

#[test]
fn source_picker() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(super::picker::WIDTH, 420.0))
        .build_ui_state(
            |ui, chosen: &mut Source| {
                dc_ui_kit::widgets::panel_frame()
                    .inner_margin(popover::Kind::Source.padding())
                    .show(ui, |ui| {
                        ui.set_width(super::picker::WIDTH - 34.0);
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
        "Микрофон (6- Usb Audio Device)".to_owned(),
        "Массив микрофонов (Realtek(R) Audio)".to_owned(),
        "Головной телефон (Гарнитура HyperX Cloud II Wireless Hands-Free AG Audio)".to_owned(),
    ];
    let start = dc_capture::RecordingConfig {
        microphone: Some(mics[0].clone()),
        ..AppSettings::default().recording
    };
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(460.0, 330.0))
        .build_ui_state(
            move |ui, recording: &mut dc_capture::RecordingConfig| {
                dc_ui_kit::widgets::panel_frame().show(ui, |ui| {
                    ui.set_width(super::menus::sound_width(ui.ctx(), &mics) - 26.0);
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
    harness
        .get_by_label("Микрофон (6- Usb Audio Device)")
        .click();
    harness.run();
    assert_eq!(
        harness.state().microphone.as_deref(),
        Some("Микрофон (6- Usb Audio Device)")
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

#[test]
fn closes_on_focus_loss_only_after_having_focus() {
    let mut open = popover::Open::new(popover::Kind::More, 0.0);
    assert!(!open.focus_lost(Some(false)), "opened behind another app");
    assert!(!open.focus_lost(None));
    assert!(!open.focus_lost(Some(true)));
    assert!(open.focus_lost(Some(false)));
}

#[test]
fn every_popover_has_its_own_title() {
    let kinds = [
        popover::Kind::Source,
        popover::Kind::Sound,
        popover::Kind::Timing,
        popover::Kind::More,
        popover::Kind::Cancel,
        popover::Kind::Quit,
        popover::Kind::Failed,
        popover::Kind::Crashed,
        popover::Kind::WhatsNew,
    ];
    let mut titles: Vec<&str> = kinds.iter().map(|k| k.title()).collect();
    titles.sort_unstable();
    titles.dedup();
    assert_eq!(titles.len(), kinds.len());
}

/// The window is transparent and draws its own shadow (decision 020); the system shadow fell on
/// the HUD 8 px below and darkened it (live check of stage 6). The drawn one ends at the bar.
#[test]
fn popover_window_is_transparent_and_its_panel_sits_above_the_bar() {
    let open = popover::Open::new(popover::Kind::Sound, 100.0);
    let bar = egui::Rect::from_min_size(egui::pos2(500.0, 900.0), egui::vec2(400.0, 62.0));
    let builder = popover::builder(open, 300.0, bar);
    assert_eq!(builder.has_shadow, Some(false));
    assert_eq!(builder.decorations, Some(false));
    assert_eq!(builder.transparent, Some(true));
    let start = popover::window_rect(open, egui::vec2(300.0, 40.0), bar);
    assert_eq!(builder.position, Some(start.min));
    assert_eq!(builder.inner_size, Some(start.size()));

    let window = popover::window_rect(open, egui::vec2(300.0, 120.0), bar);
    let panel = crate::borderless::panel_rect(window);
    let hud = crate::borderless::panel_rect(bar);
    assert_eq!(panel.size(), egui::vec2(300.0, 120.0));
    assert_eq!(panel.bottom(), hud.top() - 8.0);
    assert_eq!(panel.left(), bar.left() + 100.0 - 8.0);
    assert_eq!(
        window.bottom(),
        hud.top(),
        "the window ends at the bar, not over it"
    );
}

#[test]
fn the_more_menu_ends_where_its_button_ends() {
    let bar = egui::Rect::from_min_size(egui::pos2(500.0, 900.0), egui::vec2(400.0, 62.0));
    let anchor = 360.0;
    let more = popover::Open::new(popover::Kind::More, anchor);
    let window = popover::window_rect(more, egui::vec2(260.0, 120.0), bar);
    let panel = crate::borderless::panel_rect(window);
    let button_right = bar.left() + anchor + dc_ui_kit::tokens::CONTROL;
    assert_eq!(
        panel.right(),
        button_right + 8.0,
        "the button's right edge and the gap"
    );
    assert!(panel.right() <= crate::borderless::panel_rect(bar).right() + 8.0);
}
