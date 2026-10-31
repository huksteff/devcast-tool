//! The inspector frame (7.13): picking an object opens its panel, a tab opens its own, groups
//! remember open or closed in the settings, «Сбросить» is one undo step; the «Показ клавиш»
//! panel edits the project as `devcast keys …` does.

use std::time::Instant;

use dc_core::{History, Project};
use dc_fx_keys::Keys;
use dc_ops::{Registry, Step};
use dc_project::Defaults;
use dc_ui_kit::strings::{editor as text, inspector as names};
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};

use crate::editor::editing_tests::json;
use crate::editor::selection::{Item, Selected};
use crate::editor::tests::{State, harness, project};
use crate::editor::view::Panel;
use crate::editor::{Action, SIZE, Way};
use crate::settings::InspectorGroups;

type H = Harness<'static, State>;

fn started() -> H {
    let mut harness = harness(SIZE);
    harness.run();
    harness
}

/// The step `devcast <line>` parses.
fn step(line: &str) -> Step {
    let registry = Registry::devcast().unwrap();
    let words: Vec<String> = line.split(' ').map(str::to_owned).collect();
    let (info, taken) = registry.resolve(&words).unwrap();
    Step::new(
        info.name,
        dc_ops::parse_args(info, &words[taken..]).unwrap(),
    )
}

/// The test project after `devcast <line>` for each line, one plan.
fn cli(lines: &[&str]) -> Project {
    let registry = Registry::devcast().unwrap();
    let steps: Vec<Step> = lines.iter().map(|line| step(line)).collect();
    let mut project = project();
    dc_ops::apply(
        &registry,
        &mut project,
        &mut History::default(),
        &steps,
        false,
    )
    .unwrap();
    project
}

fn project_json(harness: &H) -> serde_json::Value {
    json(harness.state().editor.doc.project())
}

fn undo(harness: &mut H) {
    let doc = &mut harness.state_mut().editor.doc;
    doc.travel(Way::Undo, Instant::now());
    harness.run();
}

fn button(harness: &mut H, pos: Pos2, pressed: bool) {
    harness.event(Event::PointerMoved(pos));
    harness.event(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    });
    harness.run();
}

fn first_zoom(harness: &H) -> Item {
    let tracks = &harness.state().editor.tracks;
    let row = tracks
        .rows
        .iter()
        .find(|t| t.row == crate::editor::timeline::Row::Zooms);
    row.and_then(|t| t.regions.first())
        .and_then(|r| r.item)
        .unwrap()
}

fn pick(harness: &mut H, item: Option<Selected>) {
    let selection = &mut harness.state_mut().editor.view.selection;
    selection.clear();
    if let Some(item) = item {
        selection.only(item);
    }
    harness.run();
}

#[test]
fn picking_one_object_opens_its_panel_and_a_tab_opens_its_own() {
    let mut harness = started();
    assert!(
        harness
            .query_by_role_and_label(egui::Role::Switch, names::KEYS_SHOW)
            .is_some()
    );
    let zoom = Selected::Region(first_zoom(&harness));
    pick(&mut harness, Some(zoom));
    assert_eq!(harness.state().editor.view.object, Some(zoom));
    let head = harness.get_by_role_and_label(egui::Role::Heading, text::ZOOM);
    assert_eq!(head.value().as_deref(), Some("0:05.00 – 0:12.00 · 7,0 с"));
    assert!(
        harness
            .query_by_role_and_label(egui::Role::Switch, names::KEYS_SHOW)
            .is_none()
    );
    harness.snapshot("inspector_object");
    harness
        .get_by_role_and_label(egui::Role::Button, text::KEYS)
        .click();
    harness.run();
    assert_eq!(harness.state().editor.view.object, None);
    assert!(
        harness
            .query_by_role_and_label(egui::Role::Switch, names::KEYS_SHOW)
            .is_some()
    );
    pick(&mut harness, None);
    pick(&mut harness, Some(zoom));
    assert_eq!(
        harness.state().editor.view.object,
        Some(zoom),
        "a new pick opens it again"
    );
    pick(&mut harness, None);
    assert_eq!(
        harness.state().editor.view.object,
        None,
        "nothing picked: the tab"
    );
    assert_eq!(harness.state().editor.view.panel, Panel::Keys);
}

#[test]
fn tabs_without_their_panel_wait_for_their_task() {
    let harness = started();
    for title in [text::ZOOM, text::CURSOR, text::CAPTIONS, text::SOUND] {
        let mut tabs = harness.query_all_by_role_and_label(egui::Role::Button, title);
        assert!(tabs.any(|t| t.accesskit_node().is_disabled()), "{title}");
    }
}

#[test]
fn the_delete_button_removes_the_object_as_the_cli_does() {
    let mut harness = started();
    let zoom = first_zoom(&harness);
    pick(&mut harness, Some(Selected::Region(zoom)));
    harness.get_by_label(names::DELETE).click();
    harness.run();
    let Item::Zoom(id) = zoom else { unreachable!() };
    assert_eq!(
        project_json(&harness),
        json(&cli(&[&format!("zoom rm {id}")]))
    );
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&project()));
}

#[test]
fn groups_remember_open_or_closed_in_the_settings() {
    let mut harness = started();
    assert!(
        harness
            .query_by_role_and_label(egui::Role::Slider, names::KEYS_HOLD)
            .is_none(),
        "closed at first"
    );
    harness.get_by_label(text::FINE).click();
    harness.run();
    harness.get_by_role_and_label(egui::Role::Slider, names::KEYS_HOLD);
    let groups = harness.state().groups.clone();
    assert_eq!(groups.0.get("keys.fine"), Some(&true));
    harness.get_by_label(text::BASIC).click();
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(egui::Role::Switch, names::KEYS_SHOW)
            .is_none()
    );
    harness.snapshot("inspector_keys_fine");
    // Another editor with the settings of this one opens the same groups.
    let saved = harness.state().groups.clone();
    let mut again = harness_with_groups(saved);
    again.get_by_role_and_label(egui::Role::Slider, names::KEYS_HOLD);
    assert!(
        again
            .query_by_role_and_label(egui::Role::Switch, names::KEYS_SHOW)
            .is_none()
    );
    // Back to the default: the settings forget the group.
    again.get_by_label(text::FINE).click();
    again.run();
    assert_eq!(again.state().groups.0.get("keys.fine"), None);
}

fn harness_with_groups(groups: InspectorGroups) -> H {
    let mut harness = harness(SIZE);
    harness.state_mut().groups = groups;
    harness.run();
    harness.run();
    harness
}

#[test]
fn keys_panel_edits_are_the_keys_command() {
    let mut harness = started();
    harness
        .get_by_role_and_label(egui::Role::Switch, names::KEYS_SHOW)
        .click();
    harness.run();
    harness.get_by_label(names::TOP_RIGHT).click();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::SpinButton, names::KEYS_SIZE)
        .set_value_accesskit(1.5);
    harness.run();
    let lines = [
        "keys --show true",
        "keys --position top_right",
        "keys --size 1.5",
    ];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    harness.snapshot("inspector_keys");
    undo(&mut harness);
    assert_eq!(
        project_json(&harness),
        json(&cli(&lines[..2])),
        "each edit is one step"
    );
}

#[test]
fn a_dragged_slider_is_one_step_on_release() {
    let mut harness = started();
    let rail = harness
        .get_by_role_and_label(egui::Role::Slider, names::KEYS_SIZE)
        .rect();
    button(&mut harness, rail.left_center(), true);
    for share in [0.25, 0.5, 1.0] {
        harness.event(Event::PointerMoved(
            rail.lerp_inside(egui::vec2(share, 0.5)),
        ));
        harness.run();
        assert_eq!(
            project_json(&harness),
            json(&project()),
            "nothing until released"
        );
    }
    button(&mut harness, rail.right_center(), false);
    assert_eq!(project_json(&harness), json(&cli(&["keys --size 3"])));
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&project()));
}

#[test]
fn reset_brings_a_group_to_the_defaults_in_one_step() {
    let mut harness = started();
    let fine_reset = names::reset_group(text::FINE);
    let basic_reset = names::reset_group(text::BASIC);
    assert!(
        harness
            .get_by_label(&basic_reset)
            .accesskit_node()
            .is_disabled()
    );
    let edits = [
        "keys --show true",
        "keys --position top",
        "keys --size 2",
        "keys --duration 3",
    ];
    let edited = cli(&edits);
    let steps = edits.iter().map(|line| step(line)).collect();
    harness.state_mut().editor.ask(Action::Apply(steps));
    harness.run();
    assert_eq!(project_json(&harness), json(&edited));
    harness.get_by_label(&basic_reset).click();
    harness.run();
    let reset = "keys --show false --position bottom --size 1";
    let mut lines = edits.to_vec();
    lines.push(reset);
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    assert!(
        harness
            .get_by_label(&basic_reset)
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        !harness
            .get_by_label(&fine_reset)
            .accesskit_node()
            .is_disabled()
    );
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&edited), "one step back");
}

#[test]
fn reset_takes_the_users_defaults() {
    let mut harness = started();
    let mut styled = Project::default();
    let mut history = History::default();
    let registry = Registry::devcast().unwrap();
    let look = step("keys --size 2.5");
    dc_ops::apply(&registry, &mut styled, &mut history, &[look], false).unwrap();
    harness.state_mut().editor.defaults = Defaults::from_project(&styled);
    harness.run();
    harness
        .get_by_label(&names::reset_group(text::BASIC))
        .click();
    harness.run();
    let keys = harness
        .state()
        .editor
        .doc
        .project()
        .state::<Keys>()
        .unwrap();
    assert_eq!(keys.settings.size, 2.5);
    assert_eq!(project_json(&harness), json(&cli(&["keys --size 2.5"])));
}

#[test]
fn a_typed_number_applies_on_enter() {
    let mut harness = started();
    let spin = egui::Role::SpinButton;
    harness
        .get_by_role_and_label(spin, names::KEYS_SIZE)
        .focus();
    harness.run();
    let field = harness.get_by_role_and_label(spin, names::KEYS_SIZE);
    field.type_text("2");
    harness.run();
    assert_eq!(
        project_json(&harness),
        json(&project()),
        "nothing while typing"
    );
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(project_json(&harness), json(&cli(&["keys --size 2"])));
}
