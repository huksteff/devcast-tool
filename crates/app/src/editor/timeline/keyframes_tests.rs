//! Keyframes in the editor (7.25): I puts a key at the playhead and picks it, Ctrl+C and Ctrl+V
//! copy a key to the playhead (into the picked annotation), the tooltip of a marker names what
//! the key changes. Each is the project `devcast` gives for the same commands, in one undo step.

use dc_core::{OutputTime, PixelSize};
use dc_fx_annotations::{AnnotId, Annotations};
use eframe::egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable;

use super::clips_tests::{H, press, project_json, started, undo_is_one_step};
use super::key_tip::tip;
use super::regions_tests::{annotation, at, by_cli, steps, words};
use crate::editor::Action;
use crate::editor::Way;
use crate::editor::editing_tests::json;
use crate::editor::selection::{Item, Selected, same_moment};

fn apply(harness: &mut H, lines: &[&str]) {
    let lines: Vec<Vec<String>> = lines.iter().map(|l| words(l)).collect();
    harness.state_mut().editor.ask(Action::Apply(steps(&lines)));
    harness.run();
    harness.run();
}

fn expect(harness: &H, lines: &[&str]) {
    let lines: Vec<Vec<String>> = lines.iter().map(|l| words(l)).collect();
    assert_eq!(project_json(harness), json(&by_cli(&lines)));
}

/// The one picked item is the key of `id` at `secs`.
fn picked_key(harness: &H, id: AnnotId, secs: f64) -> bool {
    match harness.state().editor.view.selection.single() {
        Some(Selected::Key { id: of, at }) => {
            of == id && same_moment(at, OutputTime::from_secs_f64(secs))
        }
        _ => false,
    }
}

/// Undo, then the project of `lines`.
fn undo_to(harness: &mut H, lines: &[&str]) {
    let now = std::time::Instant::now();
    harness.state_mut().editor.doc.travel(Way::Undo, now);
    harness.run();
    expect(harness, lines);
}

fn pick(harness: &mut H, item: Selected) {
    harness.state_mut().editor.view.selection.only(item);
    harness.run();
}

fn a(n: u32) -> AnnotId {
    AnnotId::from_number(n)
}

#[test]
fn i_puts_a_key_at_the_playhead_and_picks_it() {
    let mut harness = started();
    pick(&mut harness, Selected::Region(annotation(1)));
    at(&mut harness, 24.0);
    press(&mut harness, Key::I, Modifiers::NONE);
    expect(&harness, &["annot key add a1 --at 24"]);
    assert!(picked_key(&harness, a(1), 24.0));
    press(&mut harness, Key::I, Modifiers::NONE);
    expect(&harness, &["annot key add a1 --at 24"]);
    undo_is_one_step(&mut harness);
}

#[test]
fn a_copied_key_pastes_at_the_playhead_into_the_picked_annotation() {
    let mut harness = started();
    let base = [
        "annot key add a1 --at 24",
        "annot set a1 --x 0.2 --at 24",
        "annot add text --at 24.5 --duration 2 --track 3",
    ];
    apply(&mut harness, &base);
    pick(
        &mut harness,
        Selected::Key {
            id: a(1),
            at: OutputTime::from_secs_f64(24.0),
        },
    );
    press(&mut harness, Key::C, Modifiers::COMMAND);
    at(&mut harness, 25.0);
    press(&mut harness, Key::V, Modifiers::COMMAND);
    let own = "annot key copy a1 --at 24 --to 25";
    expect(&harness, &[base[0], base[1], base[2], own]);
    assert!(picked_key(&harness, a(1), 25.0), "the pasted key is picked");
    undo_to(&mut harness, &base);

    let state = harness.state().editor.doc.project().state::<Annotations>();
    let added = state.unwrap().annotations.last().unwrap().id;
    pick(&mut harness, Selected::Region(Item::Annotation(added)));
    press(&mut harness, Key::V, Modifiers::COMMAND);
    let into = format!("annot key copy a1 --at 24 --to 25 --into {added}");
    expect(&harness, &[base[0], base[1], base[2], &into]);
    assert!(picked_key(&harness, added, 25.0));
}

#[test]
fn the_tooltip_names_what_a_key_changes() {
    let mut harness = started();
    let lines = [
        "annot key add a1 --at 24",
        "annot key add a1 --at 26",
        "annot set a1 --x 0.2 --rotation 15 --at 26",
        "text set a1 --size 77 --at 26",
    ];
    apply(&mut harness, &lines);
    let project = harness.state().editor.doc.project().clone();
    let canvas = PixelSize {
        width: 2560,
        height: 1440,
    };
    let second = tip(&project, canvas, a(1), OutputTime::from_secs_f64(26.0)).unwrap();
    assert_eq!(second.title, "Ключ 2 · 0:26.00");
    assert_eq!(second.since, "по сравнению с ключом 1:");
    assert_eq!(
        second.lines[..2],
        ["Позиция: 256, 144 → 512, 144", "Поворот: 0° → 15°"]
    );
    let size = &second.lines[2];
    assert!(
        size.starts_with("Размер шрифта: ") && size.ends_with(" → 77"),
        "{size}"
    );
    assert_eq!(second.lines.len(), 3);
    let first = tip(&project, canvas, a(1), OutputTime::from_secs_f64(24.0)).unwrap();
    assert_eq!(first.lines, ["Без изменений"]);
}

#[test]
fn hovering_a_marker_shows_the_tooltip() {
    let mut harness = started();
    apply(
        &mut harness,
        &[
            "annot key add a1 --at 24",
            "annot key add a1 --at 26",
            "annot set a1 --x 0.2 --at 26",
        ],
    );
    let name = dc_ui_kit::strings::timeline::key_marker("0:26.00");
    let centre = harness.get_by_label(&name).rect().center();
    harness.event(eframe::egui::Event::PointerMoved(centre));
    harness.run_steps(40);
    harness.get_by_label("Ключ 2 · 0:26.00");
    harness.snapshot("editor_key_tip");
}
