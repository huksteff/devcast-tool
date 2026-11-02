//! The kinds' parts of the annotation panel (7.18): shape, blur, drawing and image edit the
//! project as `devcast shape/blur/drawing set …` and `devcast image set` do, one undo step each.

use std::path::Path;

use dc_core::{Command as _, PixelSize};
use dc_fx_annotations::AnnotId;
use dc_fx_image::{ImageSet, PictureFile};
use dc_ui_kit::strings::annotations as text;
use eframe::egui;
use egui_kittest::kittest::Queryable;

use super::annotation_tests::{given, picked};
use super::tests::{H, cli, project_json, undo};
use super::zoom_tests::{click, type_in};
use crate::editor::Action;
use crate::editor::adding;
use crate::editor::editing_tests::json;

/// Checks the project after each of `lines` and that each was one undo step back to `kept`.
fn steps_back(harness: &mut H, lines: &[&str], kept: usize) {
    assert_eq!(project_json(harness), json(&cli(lines)));
    for done in (kept..lines.len()).rev() {
        undo(harness);
        assert_eq!(
            project_json(harness),
            json(&cli(&lines[..done])),
            "one step"
        );
    }
}

#[test]
fn shape_edits_are_shape_set() {
    // The arrow a3 is at 10–14 s of the output.
    let mut harness = picked(3, 12.0);
    click(&mut harness, egui::Role::RadioButton, text::RECTANGLE);
    type_in(&mut harness, text::THICKNESS, 10.0);
    click(&mut harness, egui::Role::Switch, text::GRADIENT);
    type_in(&mut harness, text::OPACITY, 50.0);
    harness.snapshot("inspector_annotation_shape");
    let lines = [
        "shape set a3 --figure rectangle",
        "shape set a3 --width 10",
        "shape set a3 --gradient true",
        "shape set a3 --opacity 0.5",
    ];
    steps_back(&mut harness, &lines, 0);
}

#[test]
fn arrow_ends_are_shape_set_from_and_to() {
    let mut harness = picked(3, 12.0);
    harness
        .get_by_label(dc_ui_kit::strings::editor::FINE)
        .click();
    harness.run();
    type_in(&mut harness, text::TIP_Y, 25.0);
    type_in(&mut harness, text::GLOW, 10.0);
    // The default arrow runs from 0,0.5 to 1,0.5 of its box.
    let lines = ["shape set a3 --tip 1,0.25", "shape set a3 --glow 10"];
    steps_back(&mut harness, &lines, 0);
}

#[test]
fn blur_edits_are_blur_set() {
    // The blur a2 lies in the second clip: 49–64 s of the output.
    let mut harness = picked(2, 50.0);
    type_in(&mut harness, text::STRENGTH, 40.0);
    type_in(&mut harness, text::TINT_COVER, 50.0);
    type_in(&mut harness, text::RADIUS, 0.0);
    harness.snapshot("inspector_annotation_blur");
    let lines = [
        "blur set a2 --strength 40",
        "blur set a2 --tint #00000080",
        "blur set a2 --radius 0",
    ];
    steps_back(&mut harness, &lines, 0);
}

#[test]
fn drawing_edits_are_drawing_set_and_clear() {
    let mut harness = picked(2, 50.0);
    let made = [
        "annot add drawing --at 50 --duration 2 --track 1",
        "drawing add a4 --points 0.1,0.1,0.9,0.9",
    ];
    given(&mut harness, &made);
    let drawing = crate::editor::selection::Item::Annotation(AnnotId::from_number(4));
    super::tests::pick(
        &mut harness,
        Some(crate::editor::selection::Selected::Region(drawing)),
    );
    type_in(&mut harness, text::THICKNESS, 8.0);
    type_in(&mut harness, text::OPACITY, 50.0);
    harness.snapshot("inspector_annotation_drawing");
    click(&mut harness, egui::Role::Button, text::CLEAR);
    let mut lines = made.to_vec();
    lines.extend([
        "drawing set a4 --width 8",
        "drawing set a4 --opacity 0.5",
        "drawing clear a4",
    ]);
    steps_back(&mut harness, &lines, 2);
}

#[test]
fn a_picked_picture_is_image_set() {
    let mut harness = picked(2, 50.0);
    let made = ["annot add image --at 50 --duration 2 --track 1"];
    given(&mut harness, &made);
    let id = AnnotId::from_number(4);
    let item = crate::editor::selection::Item::Annotation(id);
    super::tests::pick(
        &mut harness,
        Some(crate::editor::selection::Selected::Region(item)),
    );
    let sticker = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/images/sticker.png");
    let file = adding::PictureFile {
        id,
        at: None,
        path: sticker,
    };
    harness.state_mut().editor.ask(Action::Picture(file));
    harness.run();
    let mut expected = cli(&made);
    let size = PixelSize {
        width: 160,
        height: 100,
    };
    let picture = PictureFile::new("assets/images/sticker.png".to_owned(), size, &[]);
    let set = ImageSet {
        id,
        picture,
        at: None,
    };
    set.apply(&mut expected).unwrap();
    assert_eq!(project_json(&harness), json(&expected));
    let root = harness.state().editor.doc.dir().root().to_path_buf();
    assert!(
        root.join("assets/images/sticker.png").is_file(),
        "copied in"
    );
    assert!(
        harness
            .query_by_label(&text::picture("sticker.png", 160, 100))
            .is_some()
    );
    harness.snapshot("inspector_annotation_image");
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&made)), "one step");
}
