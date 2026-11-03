//! The annotation panel (7.18), common part and text: edits are the project of `devcast text
//! set` / `annot set` / `annot anim` / `annot move` / `annot layer`, one undo step each; the key
//! strip names the key at the playhead and steps to its neighbours; at a key the edit goes into
//! that key (`--at`), elsewhere into the annotation and every key.

use dc_core::OutputTime;
use dc_fx_annotations::AnnotId;
use dc_ui_kit::strings::{annotations as text, editor, timeline as rows_text};
use eframe::egui;
use egui_kittest::kittest::{NodeT as _, Queryable};

use super::tests::{H, cli, pick, project_json, step, undo};
use super::zoom_tests::{click, tall, type_in};
use crate::editor::Action;
use crate::editor::editing_tests::json;
use crate::editor::selection::{Item, Selected};

/// The editor with annotation `number` picked and the playhead at `at` s of the output; the
/// test project's text a1 is at 23–29 s of the output (a cut and 4× before it).
pub(super) fn picked(number: u32, at: f64) -> H {
    let mut harness = tall();
    harness.state_mut().editor.view.playhead = OutputTime::from_secs_f64(at);
    let id = AnnotId::from_number(number);
    pick(&mut harness, Some(Selected::Region(Item::of(id))));
    harness
}

/// Applies `devcast <line>` for each line as one undo step, as if done before the test.
pub(super) fn given(harness: &mut H, lines: &[&str]) {
    let steps = lines.iter().map(|line| step(line)).collect();
    harness.state_mut().editor.ask(Action::Apply(steps));
    harness.run();
}

/// The text of the key strip.
fn strip(harness: &H, said: &str) {
    assert!(
        harness.query_by_label(said).is_some(),
        "the strip says «{said}»"
    );
}

#[test]
fn text_edits_are_text_set() {
    let mut harness = picked(1, 26.0);
    let head = harness.get_by_role_and_label(egui::Role::Heading, rows_text::TEXT);
    assert_eq!(
        head.value().as_deref(),
        Some("Аннотации 1 · 0:23.00 – 0:29.00")
    );
    strip(&harness, text::NO_KEYS);
    type_in(&mut harness, text::SIZE, 40.0);
    click(&mut harness, egui::Role::Button, text::BOLD);
    click(&mut harness, egui::Role::RadioButton, text::LEFT);
    harness.get_by_label(text::TEXT_HINT).focus();
    harness.run();
    harness.get_by_label(text::TEXT_HINT).type_text("Шаг");
    harness.run();
    let lines = [
        "text set a1 --size 40",
        "text set a1 --bold false",
        "text set a1 --align left",
        "text set a1 --text ТекстШаг",
    ];
    let before = json(&cli(&lines[..3]));
    assert_eq!(project_json(&harness), before, "nothing while typing");
    // A click away (another field takes the keys) applies the words.
    harness
        .get_by_role_and_label(egui::Role::SpinButton, text::SIZE)
        .focus();
    harness.run();
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    harness.snapshot("inspector_annotation_text");
    for done in (1..lines.len()).rev() {
        undo(&mut harness);
        assert_eq!(
            project_json(&harness),
            json(&cli(&lines[..done])),
            "one step"
        );
    }
}

#[test]
fn reset_keeps_the_words() {
    let mut harness = picked(1, 26.0);
    given(
        &mut harness,
        &["text set a1 --text Шаг --size 40 --italic true"],
    );
    click(
        &mut harness,
        egui::Role::Button,
        &dc_ui_kit::strings::inspector::reset_group(editor::BASIC),
    );
    let lines = [
        "text set a1 --text Шаг --size 40 --italic true",
        "text set a1 --font Inter --size 32 --bold true --italic false --underline false --align center --color #ffffffff --background #00000000",
    ];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&lines[..1])), "one step");
}

#[test]
fn keys_are_named_and_stepped_to() {
    let mut harness = picked(1, 26.0);
    given(
        &mut harness,
        &["annot key add a1 --at 24", "annot key add a1 --at 28"],
    );
    strip(&harness, &text::no_key(2));
    click(&mut harness, egui::Role::Button, text::KEY_NEXT);
    assert_eq!(
        harness.state().editor.view.playhead,
        OutputTime::from_secs_f64(28.0)
    );
    strip(&harness, &text::key_of(2, 2, "0:28.00"));
    assert!(
        harness
            .get_by_role_and_label(egui::Role::Button, text::KEY_NEXT)
            .accesskit_node()
            .is_disabled(),
        "no key after the last"
    );
    click(&mut harness, egui::Role::Button, text::KEY_PREVIOUS);
    strip(&harness, &text::key_of(1, 2, "0:24.00"));
}

#[test]
fn an_edit_at_a_key_goes_into_the_key() {
    let mut harness = picked(1, 28.0);
    let keys = ["annot key add a1 --at 24", "annot key add a1 --at 28"];
    given(&mut harness, &keys);
    strip(&harness, &text::key_of(2, 2, "0:28.00"));
    type_in(&mut harness, text::SIZE, 40.0);
    harness.get_by_label(editor::FINE).click();
    harness.run();
    type_in(&mut harness, text::ROTATION, 15.0);
    let mut lines = keys.to_vec();
    lines.extend([
        "text set a1 --size 40 --at 28",
        "annot set a1 --rotation 15 --at 28",
    ]);
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    harness.snapshot("inspector_annotation_key");
    // Off the keys an edit is the annotation's and every key's.
    harness.state_mut().editor.view.playhead = OutputTime::from_secs_f64(26.0);
    harness.run();
    type_in(&mut harness, text::SIZE, 50.0);
    lines.push("text set a1 --size 50");
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&lines[..4])), "one step");
}

#[test]
fn common_rows_are_annot_commands() {
    let mut harness = picked(1, 26.0);
    harness.get_by_label(editor::FINE).click();
    harness.run();
    let enter = harness.get_by_role_and_label(egui::Role::ComboBox, text::ENTER);
    enter.click();
    harness.run();
    harness.get_by_label(text::ANIM_FADE).click();
    harness.run();
    type_in(&mut harness, text::ENTER_TIME, 0.6);
    type_in(&mut harness, text::X, 512.0);
    type_in(&mut harness, text::WIDTH, 1280.0);
    let track = harness.get_by_role_and_label(egui::Role::ComboBox, text::TRACK);
    track.scroll_to_me();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::ComboBox, text::TRACK)
        .click();
    harness.run();
    // The row head of the timeline has the same name: the item is the one that is no label.
    let second = rows_text::annotations(2);
    let rows = harness.get_all_by_label(&second);
    let item = rows
        .into_iter()
        .find(|n| n.accesskit_node().role() != egui::accesskit::Role::Label);
    item.unwrap().click();
    harness.run();
    let lines = [
        "annot anim a1 --enter fade",
        "annot anim a1 --enter-duration 0.6",
        "annot set a1 --x 0.2",
        "annot set a1 --w 0.5",
        "annot move a1 --track 1",
    ];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    harness.snapshot("inspector_annotation_fine");
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&lines[..4])), "one step");
}

#[test]
fn typewriter_is_grey_for_a_blur() {
    let mut harness = picked(2, 3.0);
    let enter = harness.get_by_role_and_label(egui::Role::ComboBox, text::ENTER);
    enter.click();
    harness.run();
    harness.get_by_label(text::ANIM_TYPE_ONLY).click();
    harness.run();
    assert_eq!(
        project_json(&harness),
        json(&cli(&[])),
        "a blur has no letters"
    );
}

/// Where copy `id` of the project shows on the output, ms, and its row.
fn copy_at(harness: &H, id: AnnotId) -> ((i64, i64), u32) {
    let project = harness.state().editor.doc.project();
    let state = project.state::<dc_fx_annotations::Annotations>().unwrap();
    let copy = state.annotations.iter().find(|a| a.id == id).unwrap();
    let shown = dc_core::TimeMap::build(project)
        .output_span(&copy.media, copy.range)
        .unwrap();
    let ms = |t: OutputTime| (t.as_secs_f64() * 1000.0).round() as i64;
    ((ms(shown.start()), ms(shown.end())), copy.track)
}

#[test]
fn duplicate_adds_a_copy_right_after_and_picks_it() {
    let mut harness = picked(1, 26.0);
    click(&mut harness, egui::Role::Button, text::DUPLICATE);
    let copy = AnnotId::from_number(4);
    assert_eq!(copy_at(&harness, copy), ((29_000, 35_000), 0), "after a1");
    let picked = Selected::Region(Item::of(copy));
    assert_eq!(harness.state().editor.view.selection.single(), Some(picked));
    undo(&mut harness);
    let now = harness.state().editor.doc.project();
    let expected = crate::editor::editing_tests::undone(&cli(&[]), now);
    assert_eq!(json(now), expected, "one step");
}

#[test]
fn duplicate_takes_the_first_free_row_when_its_own_is_busy() {
    let mut harness = picked(1, 26.0);
    given(
        &mut harness,
        &["annot add --kind shape --at 31 --duration 2 --track 0"],
    );
    click(&mut harness, egui::Role::Button, text::DUPLICATE);
    let copy = AnnotId::from_number(5);
    assert_eq!(copy_at(&harness, copy), ((29_000, 35_000), 1));
}
