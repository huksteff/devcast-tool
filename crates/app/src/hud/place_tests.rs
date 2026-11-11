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

/// A work area of a 1920 × 1080 monitor left of the main one, its taskbar 48 high, points.
fn room() -> Option<egui::Rect> {
    Some(egui::Rect::from_min_max(
        egui::pos2(-1920.0, 120.0),
        egui::pos2(0.0, 1152.0),
    ))
}

fn panel(window: egui::Rect) -> egui::Rect {
    crate::borderless::panel_rect(window)
}

#[test]
fn a_popover_too_tall_for_the_room_above_the_bar_opens_under_it() {
    let open = popover::Open::new(popover::Kind::WhatsNew, 0.0);
    let bar = egui::Rect::from_min_size(egui::pos2(-1500.0, 320.0), egui::vec2(400.0, 62.0));
    let hud = panel(bar);
    let short = place::placed(open, egui::vec2(380.0, 120.0), bar, room());
    assert_eq!(panel(short).bottom(), hud.top() - 8.0);
    let tall = place::placed(open, egui::vec2(380.0, 500.0), bar, room());
    assert_eq!(
        panel(tall).top(),
        hud.bottom() + 8.0,
        "under the bar, not over it"
    );
    assert_eq!(panel(tall).height(), 500.0);
    let unknown = place::placed(open, egui::vec2(380.0, 500.0), bar, None);
    assert_eq!(panel(unknown).bottom(), hud.top() - 8.0);
}

/// The picker (up to 620 high) by a HUD in the middle of a short screen: neither side has the
/// room, so it lays out no taller than the roomier side and stands there inside the screen.
#[test]
fn a_popover_taller_than_either_side_is_cut_to_the_roomier_one() {
    let open = popover::Open::new(popover::Kind::Source, 0.0);
    let bar = egui::Rect::from_min_size(egui::pos2(-1500.0, 500.0), egui::vec2(400.0, 62.0));
    let hud = panel(bar);
    let room = room().unwrap();
    let above = hud.top() - 8.0 - room.top();
    let under = room.bottom() - hud.bottom() - 8.0;
    assert!(above < popover::MAX_HEIGHT && under < popover::MAX_HEIGHT);
    let tallest = place::tallest(bar, Some(room));
    assert_eq!(tallest, above.max(under));
    let window = place::placed(open, egui::vec2(640.0, tallest), bar, Some(room));
    assert_eq!(panel(window).height(), tallest);
    assert!(panel(window).top() >= room.top() && panel(window).bottom() <= room.bottom());
    assert_eq!(panel(window).top(), hud.bottom() + 8.0, "more room under");
    let wanted = place::placed(open, egui::vec2(640.0, 620.0), bar, Some(room));
    assert_eq!(panel(wanted).height(), tallest, "never past the screen");
    assert_eq!(place::tallest(bar, None), popover::MAX_HEIGHT);
}

/// The room is the work area of the HUD's monitor: a popover under a bar at the very bottom
/// stays above the taskbar, and one at a side stays on the screen.
#[test]
fn a_popover_stays_inside_the_work_area() {
    let room = room().unwrap();
    let low = egui::Rect::from_min_size(egui::pos2(-1500.0, 1080.0), egui::vec2(400.0, 62.0));
    let open = popover::Open::new(popover::Kind::Sound, 100.0);
    let window = place::placed(open, egui::vec2(300.0, 200.0), low, Some(room));
    assert!(panel(window).bottom() <= room.bottom());
    assert_eq!(panel(window).bottom(), panel(low).top() - 8.0);

    let left = egui::Rect::from_min_size(egui::pos2(-1930.0, 600.0), egui::vec2(400.0, 62.0));
    let first = popover::Open::new(popover::Kind::Source, 0.0);
    let window = place::placed(first, egui::vec2(300.0, 200.0), left, Some(room));
    assert_eq!(panel(window).left(), room.left());

    let right = egui::Rect::from_min_size(egui::pos2(-300.0, 600.0), egui::vec2(400.0, 62.0));
    let more = popover::Open::new(popover::Kind::Source, 300.0);
    let window = place::placed(more, egui::vec2(640.0, 200.0), right, Some(room));
    assert_eq!(panel(window).right(), room.right());
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
