//! The tokens, text sizes and icons of the theme in one snapshot.

use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::{Icon, tokens};
use egui::{Color32, Sense, vec2};

const SWATCHES: [Color32; 15] = [
    tokens::BG,
    tokens::SURFACE,
    tokens::SURFACE_2,
    tokens::SURFACE_3,
    tokens::BORDER,
    tokens::TEXT,
    tokens::TEXT_2,
    tokens::TEXT_3,
    tokens::ACCENT,
    tokens::ACCENT_HI,
    tokens::REC,
    tokens::REC_STRONG,
    tokens::PAUSE,
    tokens::OK,
    tokens::SCRIM,
];

fn gallery(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for color in SWATCHES {
            let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
            ui.painter().rect_filled(rect, 5, color);
        }
    });
    for (size, weight) in [
        (tokens::FS_S, Weight::Regular),
        (tokens::FS_M, Weight::Regular),
        (tokens::FS_M, Weight::Medium),
        (tokens::FS_L, Weight::Semibold),
        (tokens::FS_XL, Weight::Bold),
    ] {
        let text = egui::RichText::new(format!("Запись экрана ёЩ {size} {weight:?}"))
            .font(fonts::text(size, weight));
        ui.label(text);
    }
    ui.label(egui::RichText::new("12:47 Ctrl+Shift+R").font(fonts::mono(tokens::FS_L)));
    ui.horizontal_wrapped(|ui| {
        for icon in Icon::ALL {
            let (rect, _) =
                ui.allocate_exact_size(vec2(tokens::ICON, tokens::ICON), Sense::hover());
            icon.paint(ui.painter(), rect, tokens::TEXT_2);
        }
    });
}

#[test]
fn theme_snapshot() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(vec2(560.0, 260.0))
        .build_ui(|ui| {
            egui::Frame::new()
                .fill(tokens::SURFACE)
                .inner_margin(tokens::S3)
                .show(ui, gallery);
        });
    harness.run();
    harness.snapshot("theme");
}

#[test]
fn icons_scale_with_their_rect() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(vec2(200.0, 80.0))
        .build_ui(|ui| {
            ui.horizontal(|ui| {
                for side in [16.0, 32.0, 48.0] {
                    let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
                    Icon::Gear.paint(ui.painter(), rect, tokens::TEXT);
                }
            });
        });
    harness.run();
    harness.snapshot("icons_scale");
}

#[test]
fn editor_icons() {
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(vec2(420.0, 100.0))
        .build_ui(|ui| {
            egui::Frame::new()
                .fill(tokens::SURFACE)
                .inner_margin(tokens::S3)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for icon in &Icon::ALL[27..] {
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
                            icon.paint(ui.painter(), rect, tokens::TEXT);
                        }
                    });
                });
        });
    harness.run();
    harness.snapshot("editor_icons");
}
