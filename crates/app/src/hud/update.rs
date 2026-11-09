//! The «Обновление» popover over the HUD (8.2, stage 8 mockup): «Скачать?» about a newer
//! version, its download, «Перезапустить» once it is downloaded, or why it failed.

use dc_ops::live::UpdateState;
use dc_ui_kit::strings::{kit, update};
use dc_ui_kit::widgets::{Answer, Choice, Chosen, Confirm, ErrorAction, ErrorPanel, Kind};
use eframe::egui::Ui;

use crate::app::Shared;

pub fn ui(ui: &mut Ui, shared: &mut Shared) {
    match shared.updates.state.clone() {
        UpdateState::Available { version } => available(ui, shared, &version),
        UpdateState::Downloading { version, done } => {
            let text = update::downloading(&version, done);
            let panel = ErrorPanel {
                title: update::TITLE,
                text: &text,
                extra: &[],
            };
            if panel.show(ui).is_some() {
                shared.popover = None;
            }
        }
        UpdateState::Ready { version } => ready(ui, shared, &version),
        UpdateState::Failed { error } => {
            let panel = ErrorPanel {
                title: update::TITLE,
                text: &error,
                extra: &[],
            };
            if matches!(
                panel.show(ui),
                Some(ErrorAction::Close | ErrorAction::Extra(_))
            ) {
                shared.popover = None;
            }
        }
        _ => shared.popover = None,
    }
}

/// «Скачать · Пропустить версию · Не сейчас»; «Не сейчас» asks again in 3 hours.
fn available(ui: &mut Ui, shared: &mut Shared, version: &str) {
    let text = update::available(version);
    let question = Choice {
        title: update::TITLE,
        text: &text,
        cancel: update::NOT_NOW,
        other: update::SKIP,
        confirm: update::DOWNLOAD,
    };
    match question.show(ui) {
        Some(Chosen::Confirm) => shared.download_update(),
        Some(Chosen::Other) => shared.skip_update(),
        Some(Chosen::Cancel) => shared.popover = None,
        None => {}
    }
}

/// «Перезапустить» now, or «Закрыть»: the update installs at the exit anyway.
fn ready(ui: &mut Ui, shared: &mut Shared, version: &str) {
    let text = update::ready(version);
    let question = Confirm {
        title: update::TITLE,
        text: &text,
        cancel: kit::CLOSE,
        confirm: update::RESTART,
        kind: Kind::Primary,
    };
    match question.show(ui) {
        Some(Answer::Confirm) => shared.restart_into_update(),
        Some(Answer::Cancel) => shared.popover = None,
        None => {}
    }
}
