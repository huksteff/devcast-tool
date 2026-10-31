//! The callout panel (7.16): a callout event picked on the «Курсор» row edits `callout set`
//! and the look of every callout `callout style` — each edit one undo step, the same project as
//! `devcast`; «Сбросить» of the style is one step and keeps the text of «Вкл».

use dc_fx_callout::{Callout, EventId};
use dc_ui_kit::strings::{inspector as names, kit};
use eframe::egui;
use egui_kittest::kittest::{NodeT as _, Queryable};

use super::tests::{H, cli, pick, project_json, undo};
use super::zoom_tests::{click, tall, type_in};
use crate::editor::editing_tests::json;
use crate::editor::selection::{Item, Selected};

fn picked(number: u32) -> H {
    let mut harness = tall();
    let id = EventId::from_number(number);
    pick(&mut harness, Some(Selected::Region(Item::Callout(id))));
    harness
}

/// Types `typed` into the callout text field and presses Enter; the text the field holds then.
fn type_text(harness: &mut H, typed: &str) -> String {
    harness.get_by_label(names::CALLOUT_TEXT_HINT).focus();
    harness.run();
    harness
        .get_by_label(names::CALLOUT_TEXT_HINT)
        .type_text(typed);
    harness.run();
    let value = harness.get_by_label(names::CALLOUT_TEXT_HINT).value();
    harness.key_press(egui::Key::Enter);
    harness.run();
    value.unwrap_or_default()
}

#[test]
fn event_edits_are_callout_set() {
    let mut harness = picked(2);
    let head = harness.get_by_role_and_label(egui::Role::Heading, names::CALLOUT_TEXT);
    // 46.5 s of the recording: 5 s of cut and 6 s saved by 4× earlier.
    assert_eq!(
        head.value().as_deref(),
        Some("событие 0:35.50 · печать 5,0 с")
    );
    let typed = type_text(&mut harness, "!");
    assert!(typed.ends_with('!'), "{typed}");
    assert!(!typed.contains(' '), "one word for the command line below");
    type_in(&mut harness, names::CALLOUT_TIME, 2.0);
    click(&mut harness, egui::Role::RadioButton, names::CALLOUT_HIDE);
    let text_line = format!("callout set co2 --text {typed}");
    let lines = [
        text_line.as_str(),
        "callout set co2 --duration 2",
        "callout set co2 --action off",
    ];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    assert!(
        harness.query_by_label(names::CALLOUT_TEXT_HINT).is_none(),
        "«Выкл» has no text"
    );
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&lines[..2])), "one step");
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&lines[..1])), "one step");
}

#[test]
fn a_text_left_by_picking_another_event_applies() {
    let mut harness = picked(2);
    harness.get_by_label(names::CALLOUT_TEXT_HINT).focus();
    harness.run();
    harness
        .get_by_label(names::CALLOUT_TEXT_HINT)
        .type_text("!");
    harness.run();
    let typed = harness
        .get_by_label(names::CALLOUT_TEXT_HINT)
        .value()
        .unwrap_or_default();
    assert_eq!(
        project_json(&harness),
        json(&cli(&[])),
        "nothing while typing"
    );
    let other = Selected::Region(Item::Callout(EventId::from_number(1)));
    pick(&mut harness, Some(other));
    let line = format!("callout set co2 --text {typed}");
    assert_eq!(project_json(&harness), json(&cli(&[line.as_str()])));
    let field = harness.get_by_label(names::CALLOUT_TEXT_HINT);
    assert_eq!(
        field.value().as_deref(),
        Some(""),
        "the field of «Вкл» is its own"
    );
}
#[test]
fn an_on_event_takes_the_text_of_every_on() {
    let mut harness = picked(1);
    let typed = type_text(&mut harness, "Готово");
    assert_eq!(typed, "Готово");
    click(&mut harness, egui::Role::RadioButton, names::CALLOUT_TYPE);
    let lines = [
        "callout style --text Готово",
        "callout set co1 --action text --text Готово",
    ];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    undo(&mut harness);
    click(&mut harness, egui::Role::RadioButton, names::CALLOUT_TYPE);
    assert_eq!(project_json(&harness), json(&cli(&lines)));
}

#[test]
fn without_a_text_of_on_a_new_text_event_gets_a_word() {
    let mut harness = picked(3);
    click(&mut harness, egui::Role::RadioButton, names::CALLOUT_TYPE);
    let line = format!(
        "callout set co3 --action text --text {}",
        names::CALLOUT_NEW_TEXT
    );
    assert_eq!(project_json(&harness), json(&cli(&[line.as_str()])));
}

#[test]
fn style_edits_are_callout_style_and_its_reset_one_step() {
    let mut harness = picked(1);
    let reset = names::reset_group(names::CALLOUT_STYLE);
    assert!(harness.get_by_label(&reset).accesskit_node().is_disabled());
    type_text(&mut harness, "Готово");
    type_in(&mut harness, names::CALLOUT_SIZE, 1.5);
    click(&mut harness, egui::Role::Switch, names::CALLOUT_GRADIENT);
    click(&mut harness, egui::Role::Button, kit::ADD_COLOR);
    type_in(&mut harness, names::CALLOUT_GLOW, 14.0);
    let lines = [
        "callout style --text Готово",
        "callout style --size 1.5",
        "callout style --gradient true",
        "callout style --colors #14b8c4ff,#a855f7ff,#a855f7ff",
        "callout style --glow 14",
    ];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    harness.snapshot("inspector_callout");
    harness.get_by_label(&reset).click();
    harness.run();
    let state = harness
        .state()
        .editor
        .doc
        .project()
        .state::<Callout>()
        .unwrap();
    let factory = dc_fx_callout::CalloutSettings {
        text: "Готово".to_owned(),
        ..Default::default()
    };
    assert_eq!(state.settings, factory, "the look is back, the text stays");
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&lines)), "one step");
}
