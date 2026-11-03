//! The crop dialog in the editor (7.20; the dialog is `dc_fx_crop::dialog`): it opens from the
//! preview toolbar and the «Клипы» header, «Применить» is `devcast crop` + `devcast aspect` as
//! one undo step, «Отмена», ✕ and Esc change nothing, the frame drags inside the source and
//! keeps a fixed format.

use std::time::Instant;

use dc_core::{ClipId, History, Project, Rect};
use dc_ops::{Registry, Step};
use dc_ui_kit::strings::{crop as text, editor as names, kit, timeline as rows};
use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use crate::editor::editing_tests::json;
use crate::editor::selection::Selected;
use crate::editor::tests::{State, harness, project};
use crate::editor::{SIZE, Way};

type H = Harness<'static, State>;

/// The test project after `devcast <line>` for each line.
fn cli(lines: &[&str]) -> Project {
    let registry = Registry::devcast().unwrap();
    let steps: Vec<Step> = lines
        .iter()
        .map(|line| {
            let words: Vec<String> = line.split(' ').map(str::to_owned).collect();
            let (info, taken) = registry.resolve(&words).unwrap();
            Step::new(
                info.name,
                dc_ops::parse_args(info, &words[taken..]).unwrap(),
            )
        })
        .collect();
    let mut project = project();
    let mut history = History::default();
    dc_ops::apply(&registry, &mut project, &mut history, &steps, false).unwrap();
    project
}

fn now_json(harness: &H) -> serde_json::Value {
    json(harness.state().editor.doc.project())
}

/// The editor with the dialog opened by «Кроп и формат» over the preview.
fn opened() -> H {
    let mut harness = harness(SIZE);
    harness.run();
    harness.get_by_label(names::CROP).click();
    harness.run();
    harness.run();
    assert!(harness.state().editor.crop.is_some(), "the dialog opens");
    harness
}

fn press(harness: &mut H, label: &str) {
    harness.get_by_label(label).click();
    harness.run();
}

fn type_in(harness: &mut H, label: &str, value: f64) {
    harness
        .get_by_role_and_label(egui::Role::SpinButton, label)
        .set_value_accesskit(value);
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
}

fn drag(harness: &mut H, from: Pos2, to: Pos2) {
    button(harness, from, true);
    harness.run();
    for share in [0.25, 0.5, 0.75, 1.0] {
        harness.event(Event::PointerMoved(from + (to - from) * share));
        harness.run();
    }
    button(harness, to, false);
    harness.run();
}

/// The crop of clip c1 in the project.
fn crop_of_first(harness: &H) -> Rect {
    harness.state().editor.doc.project().clips[0].crop
}

#[test]
fn cancel_close_and_esc_change_nothing() {
    let before = json(&project());
    let mut harness = opened();
    harness.snapshot("crop_dialog");
    type_in(&mut harness, text::X, 256.0);
    press(&mut harness, text::CANCEL);
    assert!(harness.state().editor.crop.is_none());
    for close in [kit::CLOSE, ""] {
        harness.get_by_label(names::CROP).click();
        harness.run();
        harness.run();
        press(&mut harness, names::ASPECT_4_3);
        match close {
            "" => {
                harness.key_press(Key::Escape);
                harness.run();
            }
            label => press(&mut harness, label),
        }
        assert!(harness.state().editor.crop.is_none(), "{close:?} closes");
    }
    assert_eq!(now_json(&harness), before);
    assert!(
        harness.state().editor.doc.next(Way::Undo).is_none(),
        "no step"
    );
}

#[test]
fn apply_is_crop_and_aspect_in_one_step() {
    let mut harness = opened();
    press(&mut harness, names::ASPECT_FREE);
    // The size first: the corner of the whole frame cannot move.
    type_in(&mut harness, text::WIDTH, 2048.0);
    type_in(&mut harness, text::HEIGHT, 1152.0);
    type_in(&mut harness, text::X, 256.0);
    type_in(&mut harness, text::Y, 144.0);
    harness.snapshot("crop_dialog_free");
    press(&mut harness, text::APPLY);
    assert!(harness.state().editor.crop.is_none());
    let lines = ["crop c1 0.1,0.1,0.8,0.8", "aspect free"];
    assert_eq!(now_json(&harness), json(&cli(&lines)));
    harness
        .state_mut()
        .editor
        .doc
        .travel(Way::Undo, Instant::now());
    harness.run();
    assert_eq!(now_json(&harness), json(&project()), "one step");
}

#[test]
fn a_fixed_format_keeps_its_proportion_while_dragging() {
    let mut harness = opened();
    press(&mut harness, names::ASPECT_4_3);
    let view = harness.get_by_label(text::FRAME).rect();
    // 4:3 of 2560 × 1440 is 1920 × 1440 in the middle: its bottom-right corner.
    let scale = view.width() / 2560.0;
    let corner = pos2(view.center().x + 960.0 * scale, view.bottom());
    drag(&mut harness, corner, corner - egui::vec2(200.0, 40.0));
    harness.snapshot("crop_dialog_dragged");
    press(&mut harness, text::APPLY);
    let crop = crop_of_first(&harness);
    let px = crop.to_pixels(dc_core::PixelSize {
        width: 2560,
        height: 1440,
    });
    assert!(dc_core::project::crop_problem(crop).is_none(), "{crop:?}");
    assert!((px[2] / px[3] - 4.0 / 3.0).abs() < 0.01, "{px:?}");
    assert!(
        px[2] < 1920.0 - 200.0 && px[0] == 320.0 && px[1] == 0.0,
        "{px:?}"
    );
    let shown = format!(
        "crop c1 {},{},{},{}",
        crop.x(),
        crop.y(),
        crop.w(),
        crop.h()
    );
    assert_eq!(now_json(&harness), json(&cli(&[&shown, "aspect 4:3"])));
}

#[test]
fn an_edge_drags_alone_and_stops_at_the_least_size() {
    let mut harness = opened();
    press(&mut harness, names::ASPECT_FREE);
    let view = harness.get_by_label(text::FRAME).rect();
    let edge = view.right_center();
    drag(&mut harness, edge, pos2(view.left() - 50.0, edge.y));
    press(&mut harness, text::APPLY);
    let crop = crop_of_first(&harness);
    assert_eq!((crop.x(), crop.y(), crop.h()), (0.0, 0.0, 1.0));
    assert_eq!(crop.w(), dc_core::project::CROP_MIN, "{crop:?}");
}

#[test]
fn the_clips_header_opens_the_picked_clip() {
    let mut harness = harness(SIZE);
    harness.run();
    let second = ClipId::from_number(2);
    harness
        .state_mut()
        .editor
        .view
        .selection
        .only(Selected::Clip(second));
    harness.run();
    harness.get_by_label(rows::CLIP_CROP).click();
    harness.run();
    harness.run();
    let dialog = &harness.state().editor.crop.as_ref().unwrap().dialog;
    assert_eq!(dialog.clip(), second);
    let size = dialog.size();
    assert_eq!((size.width, size.height), (1920, 1080));
    // Picked 16:9 is the format of the project: the output follows the first clip.
    let output = dialog.output().unwrap();
    assert_eq!((output.width, output.height), (2560, 1440));
}
