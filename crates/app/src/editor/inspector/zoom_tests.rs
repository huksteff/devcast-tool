//! The zoom panels (7.14): the rail tab edits `zoom motion`, a picked zoom `zoom set` — each edit
//! one undo step, the same project as `devcast`; «Подобрать зумы» adds the zooms of
//! `devcast zoom add --at clicks` on the fixture recording `clicks.devcast`.

use std::path::Path;

use dc_core::{History, Op, Project};
use dc_fx_zoom::{DEFAULT_SCALE, Mode, ZoomAdd, ZoomEffect};
use dc_project::ProjectDir;
use dc_ui_kit::strings::{editor as text, inspector as names, timeline as rows_text};
use eframe::egui;
use egui_kittest::kittest::{NodeT as _, Queryable};

use super::tests::{H, cli, first_zoom, pick, project_json, undo};
use crate::editor::editing_tests::json;
use crate::editor::selection::{Item, Selected};
use crate::editor::tests::{harness_of, harness_with};
use crate::editor::{EditorWindow, SIZE};

/// The editor 1280 × 1000 with the least timeline: the whole zoom panel is in sight.
pub(super) fn tall() -> H {
    let layout = crate::settings::EditorLayout {
        timeline_height: 140.0,
        ..Default::default()
    };
    let mut harness = harness_with(egui::vec2(1280.0, 1000.0), layout);
    harness.run();
    harness
}

fn zoom_tab(harness: &mut H) {
    harness
        .get_by_role_and_label(egui::Role::Button, text::ZOOM)
        .click();
    harness.run();
}

pub(super) fn click(harness: &mut H, role: egui::Role, label: &str) {
    harness.get_by_role_and_label(role, label).click();
    harness.run();
}

pub(super) fn type_in(harness: &mut H, label: &str, value: f64) {
    harness
        .get_by_role_and_label(egui::Role::SpinButton, label)
        .set_value_accesskit(value);
    harness.run();
}

#[test]
fn zoom_tab_edits_are_zoom_motion() {
    let mut harness = tall();
    zoom_tab(&mut harness);
    click(&mut harness, egui::Role::RadioButton, names::ZOOM_SMOOTH);
    click(&mut harness, egui::Role::Switch, names::ZOOM_CONNECT);
    click(&mut harness, egui::Role::Switch, names::ZOOM_AUTO);
    harness.get_by_label(text::FINE).click();
    harness.run();
    type_in(&mut harness, names::ZOOM_BLUR, 1.0);
    type_in(&mut harness, names::ZOOM_IN, 0.5);
    let lines = [
        "zoom motion --preset smooth",
        "zoom motion --connect false",
        "zoom motion --auto false",
        "zoom motion --blur 1",
        "zoom motion --zoom-in 0.5",
    ];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    // Durations of no preset: neither is chosen; «Focused» brings its durations.
    harness.snapshot("inspector_zoom");
    click(&mut harness, egui::Role::RadioButton, names::ZOOM_FOCUSED);
    let mut focused = lines.to_vec();
    focused.push("zoom motion --preset focused");
    assert_eq!(project_json(&harness), json(&cli(&focused)));
    undo(&mut harness);
    undo(&mut harness);
    assert_eq!(
        project_json(&harness),
        json(&cli(&lines[..4])),
        "each edit is one step"
    );
}

#[test]
fn zoom_tab_reset_is_one_step_to_the_defaults() {
    let mut harness = tall();
    zoom_tab(&mut harness);
    let basic = names::reset_group(text::BASIC);
    assert!(harness.get_by_label(&basic).accesskit_node().is_disabled());
    click(&mut harness, egui::Role::RadioButton, names::ZOOM_SMOOTH);
    click(&mut harness, egui::Role::Switch, names::ZOOM_AUTO);
    let edited = ["zoom motion --preset smooth", "zoom motion --auto false"];
    harness.get_by_label(&basic).click();
    harness.run();
    let mut lines = edited.to_vec();
    lines.push("zoom motion --zoom-in 0.2 --zoom-out 0.2 --connect true --auto true");
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&edited)), "one step back");
}

#[test]
fn a_picked_zoom_edits_are_zoom_set() {
    let mut harness = tall();
    let zoom = first_zoom(&harness);
    let Item::Zoom(id) = zoom else { unreachable!() };
    pick(&mut harness, Some(Selected::Region(zoom)));
    click(&mut harness, egui::Role::RadioButton, names::ZOOM_FIXED);
    type_in(&mut harness, names::ZOOM_SCALE, 2.0);
    type_in(&mut harness, names::ZOOM_X, 30.0);
    click(&mut harness, egui::Role::Switch, names::ZOOM_CONNECT);
    let lines = [
        format!("zoom set {id} --mode fixed"),
        format!("zoom set {id} --scale 2"),
        format!("zoom set {id} --x 0.3"),
        "zoom motion --connect false".to_owned(),
    ];
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    harness.get_by_label(text::FINE).click();
    harness.run();
    harness.snapshot("inspector_zoom_object");
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&lines[..3])), "one step");
    harness
        .get_by_label(&names::reset_group(text::BASIC))
        .click();
    harness.run();
    let reset = format!("zoom set {id} --scale 1.5 --mode follow --x 0.5 --y 0.5");
    let mut all = lines[..3].to_vec();
    all.push(&reset);
    assert_eq!(project_json(&harness), json(&cli(&all)));
    undo(&mut harness);
    assert_eq!(
        project_json(&harness),
        json(&cli(&lines[..3])),
        "reset is one step"
    );
}

/// The editor of a copy of `clicks.devcast` in a folder with Cyrillic and a space.
fn clicks() -> H {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings/clicks.devcast");
    let source = ProjectDir::open(fixture).unwrap();
    let project = source.load().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let dir = source
        .save_as(&project, temp.path().join("подбор зумов.devcast"))
        .unwrap();
    let (editor, problems) = EditorWindow::open(dir, Vec::new()).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    let mut harness = harness_of(temp, editor, SIZE, Default::default());
    harness.state_mut().errors = Some(Vec::new());
    harness.run();
    harness
}

/// The project of `harness` after `devcast zoom add --at clicks`: the zooms `dc_ops` suggests
/// (the CLI test `clicks_of_the_fixture_are_the_zooms_of_dc_ops` checks the CLI adds these).
fn by_clicks(harness: &H) -> Project {
    let doc = &harness.state().editor.doc;
    let mut project = doc.project().clone();
    let found = dc_ops::click_zooms(doc.dir().root(), &project).unwrap();
    assert!(!found.zooms.is_empty());
    let commands = dc_ops::click_commands(&found, DEFAULT_SCALE, Mode::default());
    let commands = dc_ops::as_commands(&commands);
    let mut history = History::default();
    dc_ops::apply_commands(&mut project, &mut history, ZoomAdd::NAME, &commands, false).unwrap();
    project
}

fn zooms(harness: &H) -> usize {
    let project = harness.state().editor.doc.project();
    project.state::<ZoomEffect>().unwrap().zooms.len()
}

#[test]
fn suggest_zooms_adds_the_zooms_of_the_clicks_in_one_step() {
    let mut harness = clicks();
    let before = harness.state().editor.doc.project().clone();
    let expected = json(&by_clicks(&harness));
    click(&mut harness, egui::Role::Button, rows_text::SUGGEST_ZOOMS);
    assert_eq!(project_json(&harness), expected);
    let added = zooms(&harness);
    assert_eq!(harness.state().editor.view.selection.len(), added, "picked");
    // Again: every cluster has its zoom already, the toast says so and nothing changes.
    click(&mut harness, egui::Role::Button, rows_text::SUGGEST_ZOOMS);
    assert_eq!(project_json(&harness), expected);
    let errors = harness.state().errors.clone().unwrap();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].starts_with("автозумов нет"), "{errors:?}");
    undo(&mut harness);
    let now = harness.state().editor.doc.project();
    let undone = crate::editor::editing_tests::undone(&before, now);
    assert_eq!(json(now), undone, "one step");
    // The panel's button does the same.
    let expected = json(&by_clicks(&harness));
    zoom_tab(&mut harness);
    click(&mut harness, egui::Role::Button, names::ZOOM_SUGGEST);
    assert_eq!(project_json(&harness), expected);
    let added = zooms(&harness);
    assert_eq!(harness.state().editor.view.selection.len(), added, "picked");
}
