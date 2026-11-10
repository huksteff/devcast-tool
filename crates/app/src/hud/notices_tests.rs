use dc_ui_kit::strings::{kit, notices, settings};
use dc_ui_kit::widgets::panel_frame;
use eframe::egui;
use egui_kittest::kittest::Queryable;

use super::*;
use crate::settings::Store;

const VERSION: &str = env!("CARGO_PKG_VERSION");

struct State {
    shared: Option<Shared>,
    dir: tempfile::TempDir,
}

fn harness(
    kind: popover::Kind,
    news: Vec<dc_update::Release>,
) -> egui_kittest::Harness<'static, State> {
    let dir = tempfile::tempdir().unwrap();
    dc_ui_kit::testing::harness()
        .with_size(egui::vec2(
            396.0,
            if news.is_empty() { 140.0 } else { 300.0 },
        ))
        .build_ui_state(
            move |ui, state: &mut State| {
                let shared = state.shared.get_or_insert_with(|| {
                    let store = Store::load(&state.dir.path().join("settings.json")).unwrap();
                    let mut shared = Shared::new(ui.ctx().clone(), store);
                    shared.popover = Some(Open::new(kind, 0.0));
                    shared.news = news.clone();
                    shared
                });
                panel_frame().show(ui, |ui| {
                    ui.set_width(356.0);
                    match kind {
                        popover::Kind::WhatsNew => whats_new(ui, shared),
                        _ => crashed(ui, shared),
                    }
                });
            },
            State { shared: None, dir },
        )
}

#[test]
fn crashed_popover() {
    let mut harness = harness(popover::Kind::Crashed, Vec::new());
    harness.run();
    harness.get_by_label(settings::OPEN_LOGS);
    harness.get_by_label(notices::CRASHED);
    harness.snapshot("hud_crashed");
    harness.get_by_label(kit::CLOSE).click();
    harness.run();
    assert!(harness.state().shared.as_ref().unwrap().popover.is_none());
}

#[test]
fn a_new_crash_report_opens_the_popover_once() {
    let dir = tempfile::tempdir().unwrap();
    let crashes = dir.path().join("logs").join("crashes");
    let store = Store::load(&dir.path().join("settings.json")).unwrap();
    let mut shared = Shared::new(egui::Context::default(), store);
    at_start(&mut shared, &crashes, VERSION).unwrap();
    assert!(shared.popover.is_none(), "no reports");
    crate::crashes::write_panic(&crashes, "паника").unwrap();
    at_start(&mut shared, &crashes, VERSION).unwrap();
    assert_eq!(shared.popover.map(|o| o.kind), Some(popover::Kind::Crashed));
    shared.popover = None;
    let reread = Store::load(&dir.path().join("settings.json")).unwrap();
    assert!(reread.get().crash_seen > 0, "remembered in settings.json");
    at_start(&mut shared, &crashes, VERSION).unwrap();
    assert!(shared.popover.is_none(), "told once");
}

fn release(version: &str, groups: &[(&str, &[&str])]) -> dc_update::Release {
    dc_update::Release {
        version: version.to_owned(),
        groups: groups
            .iter()
            .map(|(title, items)| dc_update::Group {
                title: (*title).to_owned(),
                items: items.iter().map(|i| (*i).to_owned()).collect(),
            })
            .collect(),
    }
}

#[test]
fn whats_new_popover() {
    let news = vec![release(
        VERSION,
        &[
            (
                dc_update::MAIN,
                &["Лезвие (B) на таймлайне", "Шумоподавление микрофона"],
            ),
            ("Исправлено", &["Субтитры не съезжают: это в окне не видно"]),
        ],
    )];
    let mut harness = harness(popover::Kind::WhatsNew, news);
    harness.run();
    harness.get_by_label(&notices::whats_new(VERSION));
    harness.snapshot("hud_whats_new");
    harness.get_by_label(notices::GOT_IT).click();
    harness.run();
    let shared = harness.state().shared.as_ref().unwrap();
    assert!(shared.popover.is_none() && shared.news.is_empty());
}

#[test]
fn a_new_version_is_remembered_and_the_first_run_tells_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let crashes = dir.path().join("crashes");
    let store = Store::load(&dir.path().join("settings.json")).unwrap();
    let mut shared = Shared::new(egui::Context::default(), store);
    at_start(&mut shared, &crashes, "0.1.0").unwrap();
    assert!(shared.popover.is_none() && shared.news.is_empty());
    assert_eq!(shared.store.get().version_seen.as_deref(), Some("0.1.0"));
    at_start(&mut shared, &crashes, VERSION).unwrap();
    assert_eq!(shared.store.get().version_seen.as_deref(), Some(VERSION));
    let news = dc_update::news(Some("0.1.0"), VERSION).unwrap();
    assert_eq!(shared.news, news, "the releases of CHANGELOG.md in between");
}

#[test]
fn closing_the_crash_notice_shows_the_news() {
    let mut harness = harness(
        popover::Kind::Crashed,
        vec![release(VERSION, &[("Добавлено", &["Запись"])])],
    );
    harness.run();
    harness.get_by_label(kit::CLOSE).click();
    harness.run();
    let open = harness.state().shared.as_ref().unwrap().popover;
    assert_eq!(open.map(|o| o.kind), Some(popover::Kind::WhatsNew));
}

/// The «Обновление» popover in its three looks.
fn update_popover(state: dc_ops::live::UpdateState, name: &str) {
    let dir = tempfile::tempdir().unwrap();
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(356.0, 150.0))
        .build_ui_state(
            move |ui, state: &mut State| {
                let shared = state.shared.get_or_insert_with(|| {
                    let store = Store::load(&state.dir.path().join("settings.json")).unwrap();
                    Shared::new(ui.ctx().clone(), store)
                });
                panel_frame().show(ui, |ui| {
                    ui.set_width(316.0);
                    super::super::update::ui(ui, shared);
                });
            },
            State { shared: None, dir },
        );
    harness.state_mut().shared = None;
    harness.run();
    harness.state_mut().shared.as_mut().unwrap().updates.state = state;
    harness.run();
    harness.snapshot(name);
}

#[test]
fn update_available_popover() {
    let version = "0.2.0".to_owned();
    let state = dc_ops::live::UpdateState::Available { version };
    update_popover(state, "hud_update_available");
}

#[test]
fn update_downloading_popover() {
    let version = "0.2.0".to_owned();
    let state = dc_ops::live::UpdateState::Downloading {
        version,
        done: 0.62,
    };
    update_popover(state, "hud_update_downloading");
}

#[test]
fn update_ready_popover() {
    let version = "0.2.0".to_owned();
    let state = dc_ops::live::UpdateState::Ready { version };
    update_popover(state, "hud_update_ready");
}
