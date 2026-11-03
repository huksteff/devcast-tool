//! Popovers the HUD opens at the start (stage 8 mockup): «DevCast в прошлый раз упал» when a
//! crash report is newer than the last one told about (8.5, `crashes.rs`), then «Что нового»
//! at the first start of a new version (8.3, the releases of `CHANGELOG.md` since the last run).

use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::strings::{kit, notices, settings};
use dc_ui_kit::tokens::{FS_M, FS_S, S1, S2, S3, TEXT, TEXT_2};
use dc_ui_kit::widgets::{Answer, Button, Confirm, Kind};
use eframe::egui::{self, Layout, RichText, Ui};

use crate::app::Shared;
use crate::hud::popover::{self, Open};

/// Highest the list of «Что нового» gets before it scrolls, points.
const NEWS_HEIGHT: f32 = 420.0;

/// What the start tells, the version of this program being `current`: a crash report newer than
/// the remembered one, then the releases since the version that ran last. The settings
/// remember both, so each is told once; the first run of all tells no news.
pub fn at_start(
    shared: &mut Shared,
    crashes: &std::path::Path,
    current: &str,
) -> Result<(), crate::AppError> {
    let seen = shared.store.get().clone();
    let crashed = crate::crashes::newer_than(crashes, seen.crash_seen)?;
    if seen.version_seen.as_deref() != Some(current) {
        if let Some(last) = &seen.version_seen {
            shared.news = dc_update::news(Some(last), current)?;
        }
        shared
            .store
            .update(|s| s.version_seen = Some(current.to_owned()))?;
    }
    if let Some(newest) = crashed {
        shared.store.update(|s| s.crash_seen = newest)?;
        shared.popover = Some(Open::new(popover::Kind::Crashed, 0.0));
    } else {
        shared.popover = next(shared);
    }
    Ok(())
}

/// «Что нового» while there are news.
fn next(shared: &Shared) -> Option<Open> {
    (!shared.news.is_empty()).then(|| Open::new(popover::Kind::WhatsNew, 0.0))
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
            shared.popover = next(shared);
        }
        Some(Answer::Cancel) => shared.popover = next(shared),
        None => {}
    }
}

/// The groups of each new release and «Понятно».
pub fn whats_new(ui: &mut Ui, shared: &mut Shared) {
    let current = env!("CARGO_PKG_VERSION");
    ui.label(
        RichText::new(notices::whats_new(current))
            .font(fonts::text(FS_M, Weight::Semibold))
            .color(TEXT),
    );
    ui.add_space(S2);
    egui::ScrollArea::vertical()
        .max_height(NEWS_HEIGHT)
        .show(ui, |ui| {
            let several = shared.news.len() > 1;
            for release in &shared.news {
                if several {
                    ui.label(RichText::new(&release.version).color(TEXT_2));
                }
                for group in &release.groups {
                    ui.label(
                        RichText::new(&group.title)
                            .font(fonts::text(FS_S, Weight::Semibold))
                            .color(TEXT),
                    );
                    ui.add_space(S1);
                    for item in &group.items {
                        let line = RichText::new(format!("•  {item}"));
                        ui.label(line.font(fonts::text(FS_S, Weight::Regular)).color(TEXT_2));
                    }
                    ui.add_space(S2);
                }
            }
        });
    ui.add_space(S3);
    let size = egui::vec2(ui.available_width(), dc_ui_kit::tokens::CONTROL_S);
    let got = ui.allocate_ui_with_layout(size, Layout::right_to_left(egui::Align::Center), |ui| {
        ui.add(Button::new(notices::GOT_IT).kind(Kind::Primary).small())
            .clicked()
    });
    if got.inner {
        shared.news.clear();
        shared.popover = None;
    }
}

#[cfg(test)]
#[path = "notices_tests.rs"]
mod tests;
