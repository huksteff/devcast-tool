//! The «Кроп и формат» dialog on its own, as the editor shows it: «Отмена», ✕ and Esc answer
//! nothing, «Применить» answers `crop` and `aspect` (only the changed ones), the frame drags
//! inside the source and keeps a fixed format, the output follows the first clip, the picture is
//! asked at the aspect of the source. The editor's snapshots of it are in dc-app
//! (`crop_dialog*.png`).

use dc_core::project::{CROP_MIN, Media, MediaKind};
use dc_core::{Clip, ClipId, PixelSize, Project, Range, Rect, SourceTime, Step, TimeDelta};
use dc_ui_kit::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, pos2};
use dc_ui_kit::strings::{crop as text, editor as names, kit};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::{Map, json};

use crate::dialog::{self, Answer, Dialog};
use crate::{Aspect, Crop, CropState};

struct State {
    dialog: Option<Dialog>,
    answer: Option<Answer>,
    /// The sizes the picture was asked at.
    asked: Vec<PixelSize>,
}

type H = Harness<'static, State>;

fn clip(project: &mut Project, width: u32, height: u32) {
    let id = project.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(5.0);
    project.clips.push(Clip {
        id,
        media: Media {
            path: "media/a.mp4".to_owned(),
            kind: MediaKind::Recording,
            duration,
            size: PixelSize { width, height },
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration).unwrap(),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
        audio: Default::default(),
    });
}

/// Clips c1 2560 × 1440 and c2 1920 × 1080, the format 16:9.
fn project() -> Project {
    let mut project = Project::default();
    clip(&mut project, 2560, 1440);
    clip(&mut project, 1920, 1080);
    let mut crop = CropState::default();
    crop.settings.aspect = Aspect::Wide16x9;
    project.set_state::<Crop>(&crop).unwrap();
    project
}

/// The dialog of clip `n` of [`project`], shown until it answers.
fn opened(n: u32) -> H {
    let dialog = Dialog::open(&project(), ClipId::from_number(n), "Клип".to_owned()).unwrap();
    let state = State {
        dialog: Some(dialog),
        answer: None,
        asked: Vec::new(),
    };
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, state: &mut State| {
                let Some(open) = state.dialog.as_mut() else {
                    return;
                };
                let asked = &mut state.asked;
                let answer = dialog::show(ui, open, &mut |size| {
                    asked.push(size);
                    Err(String::new())
                });
                if answer.is_some() {
                    state.answer = answer;
                    state.dialog = None;
                }
            },
            state,
        );
    harness.run();
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

fn step(op: &str, args: serde_json::Value) -> Step {
    let serde_json::Value::Object(args) = args else {
        panic!("arguments are an object");
    };
    Step::new(op, args)
}

/// The crop rect `crop` of the answer, which must be «Применить» with it first.
fn applied_rect(harness: &H) -> Rect {
    let Some(Answer::Apply(steps)) = &harness.state().answer else {
        panic!("«Применить» answers: {:?}", harness.state().answer);
    };
    assert_eq!(steps[0].op, "crop", "{steps:?}");
    serde_json::from_value(steps[0].args["rect"].clone()).unwrap()
}

#[test]
fn cancel_close_and_esc_answer_cancel() {
    for close in [text::CANCEL, kit::CLOSE, ""] {
        let mut harness = opened(1);
        type_in(&mut harness, text::X, 256.0);
        press(&mut harness, names::ASPECT_4_3);
        match close {
            "" => {
                harness.key_press(Key::Escape);
                harness.run();
            }
            label => press(&mut harness, label),
        }
        assert_eq!(harness.state().answer, Some(Answer::Cancel), "{close:?}");
    }
}

#[test]
fn apply_unchanged_answers_no_steps() {
    let mut harness = opened(1);
    assert!(harness.state().answer.is_none(), "open until answered");
    press(&mut harness, text::APPLY);
    assert_eq!(harness.state().answer, Some(Answer::Apply(Vec::new())));
}

#[test]
fn apply_answers_crop_and_aspect() {
    let mut harness = opened(1);
    press(&mut harness, names::ASPECT_FREE);
    // The size first: the corner of the whole frame cannot move.
    type_in(&mut harness, text::WIDTH, 2048.0);
    type_in(&mut harness, text::HEIGHT, 1152.0);
    type_in(&mut harness, text::X, 256.0);
    type_in(&mut harness, text::Y, 144.0);
    press(&mut harness, text::APPLY);
    let rect = Rect::new(0.1, 0.1, 0.8, 0.8).unwrap();
    let steps = vec![
        step("crop", json!({"clip": "c1", "rect": rect})),
        step("aspect", json!({"aspect": "free"})),
    ];
    assert_eq!(harness.state().answer, Some(Answer::Apply(steps)));
}

#[test]
fn a_format_alone_is_one_aspect_step() {
    let mut harness = opened(2);
    press(&mut harness, names::ASPECT_SOURCE);
    // 16:9 clip c2 in the format of 16:9 c1: the frame stays whole.
    press(&mut harness, text::APPLY);
    let mut args = Map::new();
    args.insert("aspect".to_owned(), json!("source"));
    let steps = vec![Step::new("aspect", args)];
    assert_eq!(harness.state().answer, Some(Answer::Apply(steps)));
}

#[test]
fn a_fixed_format_keeps_its_proportion_while_dragging() {
    let mut harness = opened(1);
    press(&mut harness, names::ASPECT_4_3);
    let view = harness.get_by_label(text::FRAME).rect();
    // 4:3 of 2560 × 1440 is 1920 × 1440 in the middle: its bottom-right corner.
    let scale = view.width() / 2560.0;
    let corner = pos2(view.center().x + 960.0 * scale, view.bottom());
    drag(&mut harness, corner, corner - egui::vec2(200.0, 40.0));
    press(&mut harness, text::APPLY);
    let crop = applied_rect(&harness);
    let px = crop.to_pixels(PixelSize {
        width: 2560,
        height: 1440,
    });
    assert!(dc_core::project::crop_problem(crop).is_none(), "{crop:?}");
    assert!((px[2] / px[3] - 4.0 / 3.0).abs() < 0.01, "{px:?}");
    assert!(
        px[2] < 1920.0 - 200.0 && px[0] == 320.0 && px[1] == 0.0,
        "{px:?}"
    );
}

#[test]
fn an_edge_drags_alone_and_stops_at_the_least_size() {
    let mut harness = opened(1);
    press(&mut harness, names::ASPECT_FREE);
    let view = harness.get_by_label(text::FRAME).rect();
    let edge = view.right_center();
    drag(&mut harness, edge, pos2(view.left() - 50.0, edge.y));
    press(&mut harness, text::APPLY);
    let crop = applied_rect(&harness);
    assert_eq!((crop.x(), crop.y(), crop.h()), (0.0, 0.0, 1.0));
    assert_eq!(crop.w(), CROP_MIN, "{crop:?}");
}

#[test]
fn the_output_follows_the_first_clip_and_the_picture_its_source() {
    let mut harness = opened(2);
    let dialog = harness.state().dialog.as_ref().unwrap();
    assert_eq!(dialog.clip(), ClipId::from_number(2));
    let size = dialog.size();
    assert_eq!((size.width, size.height), (1920, 1080));
    let output = dialog.output().unwrap();
    assert_eq!((output.width, output.height), (2560, 1440));
    harness.run();
    let asked = harness.state().asked.last().copied().unwrap();
    assert!(asked.width <= 1920 && asked.height <= 1080, "{asked:?}");
    let ratio = f64::from(asked.width) / f64::from(asked.height);
    assert!((ratio - 16.0 / 9.0).abs() < 0.01, "{asked:?}");
}

#[test]
fn no_dialog_without_the_clip() {
    assert!(Dialog::open(&project(), ClipId::from_number(9), String::new()).is_none());
}
