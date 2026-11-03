//! Popovers the HUD opens at the start (stage 8 mockup): «DevCast в прошлый раз упал» when a
//! crash report is newer than the last one told about (8.5, `crashes.rs`).

use dc_ui_kit::strings::{kit, notices, settings};
use dc_ui_kit::widgets::{Answer, Confirm, Kind};
use eframe::egui::Ui;

use crate::app::Shared;
use crate::hud::popover::{self, Open};

/// The popover the start opens, if any: a crash report newer than `seen` (Unix seconds) makes
/// the HUD tell about it, and the settings remember it so the next start stays quiet.
pub fn at_start(shared: &mut Shared, crashes: &std::path::Path) -> Result<(), crate::AppError> {
    let seen = shared.store.get().crash_seen;
    if let Some(newest) = crate::crashes::newer_than(crashes, seen)? {
        shared.store.update(|s| s.crash_seen = newest)?;
        shared.popover = Some(Open::new(popover::Kind::Crashed, 0.0));
    }
    Ok(())
}

/// «Закрыть» or «Открыть папку логов» (the dumps lie in it, `logs\crashes`).
pub fn crashed(ui: &mut Ui, shared: &mut Shared) {
    let question = Confirm {
        title: notices::CRASHED,
        text: notices::CRASHED_TEXT,
        cancel: kit::CLOSE,
        confirm: settings::OPEN_LOGS,
        kind: Kind::Primary,
    };
    match question.show(ui) {
        Some(Answer::Confirm) => {
            shared.report(crate::open_logs());
            shared.popover = None;
        }
        Some(Answer::Cancel) => shared.popover = None,
        None => {}
    }
}

#[cfg(test)]
#[path = "notices_tests.rs"]
mod tests;
