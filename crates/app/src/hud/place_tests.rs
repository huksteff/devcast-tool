use eframe::egui;

use crate::hud::{place, popover};

/// The window is transparent and draws its own shadow (decision 020); the system shadow fell on
/// the HUD 8 px below and darkened it (live check of stage 6). The drawn one ends at the bar.
#[test]
fn popover_window_is_transparent_and_its_panel_sits_above_the_bar() {
    let open = popover::Open::new(popover::Kind::Sound, 100.0);
    let bar = egui::Rect::from_min_size(egui::pos2(500.0, 900.0), egui::vec2(400.0, 62.0));
    let builder = popover::builder(open, 300.0, bar, None);
    assert_eq!(builder.has_shadow, Some(false));
    assert_eq!(builder.decorations, Some(false));
    assert_eq!(builder.transparent, Some(true));
    let start = place::window_rect(open, egui::vec2(300.0, 40.0), bar);
    assert_eq!(builder.position, Some(start.min));
    assert_eq!(builder.inner_size, Some(start.size()));

    let window = place::window_rect(open, egui::vec2(300.0, 120.0), bar);
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
fn a_popover_too_tall_for_the_room_above_the_bar_opens_under_it() {
    let open = popover::Open::new(popover::Kind::WhatsNew, 0.0);
    let bar = egui::Rect::from_min_size(egui::pos2(500.0, 200.0), egui::vec2(400.0, 62.0));
    let hud = crate::borderless::panel_rect(bar);
    let short = place::placed(open, egui::vec2(380.0, 120.0), bar, Some(0.0));
    assert_eq!(
        crate::borderless::panel_rect(short).bottom(),
        hud.top() - 8.0
    );
    let tall = place::placed(open, egui::vec2(380.0, 500.0), bar, Some(0.0));
    assert_eq!(
        crate::borderless::panel_rect(tall).top(),
        hud.bottom() + 8.0,
        "under the bar, not over it"
    );
    let unknown = place::placed(open, egui::vec2(380.0, 500.0), bar, None);
    assert_eq!(
        crate::borderless::panel_rect(unknown).bottom(),
        hud.top() - 8.0
    );
}

#[test]
fn the_more_menu_ends_where_its_button_ends() {
    let bar = egui::Rect::from_min_size(egui::pos2(500.0, 900.0), egui::vec2(400.0, 62.0));
    let anchor = 360.0;
    let more = popover::Open::new(popover::Kind::More, anchor);
    let window = place::window_rect(more, egui::vec2(260.0, 120.0), bar);
    let panel = crate::borderless::panel_rect(window);
    let button_right = bar.left() + anchor + dc_ui_kit::tokens::CONTROL;
    assert_eq!(
        panel.right(),
        button_right + 8.0,
        "the button's right edge and the gap"
    );
    assert!(panel.right() <= crate::borderless::panel_rect(bar).right() + 8.0);
}
