//! The cursor panels (7.15): the «Курсор» tab edits `cursor` and `clicks`, a click dot picked on
//! the «Курсор» row edits `click set` — each edit one undo step, the same project as `devcast`;
//! the dots stand at the output times of the presses, through a cut, a 4× range and two
//! recordings.

use dc_capture::input::{Button, Click};
use dc_core::{OutputTime, SourceTime};
use dc_ui_kit::strings::{editor as text, inspector as names, timeline as rows_text};
use eframe::egui;
use egui_kittest::kittest::{NodeT as _, Queryable};

use super::tests::{H, cli, project_json, undo};
use super::zoom_tests::{click, tall, type_in};
use crate::editor::editing_tests::json;
use crate::editor::lanes::click_dots;
use crate::editor::presses::recordings;
use crate::editor::selection::Selected;
use crate::editor::tests::{harness_with, project};
use crate::editor::timeline::timecode;

fn cursor_tab(harness: &mut H) {
    click(harness, egui::Role::Button, text::CURSOR);
}

fn press(at: f64) -> Click {
    Click {
        at: SourceTime::from_secs_f64(at),
        button: Button::Left,
        x: 100,
        y: 100,
        double: false,
    }
}

/// Presses of the first recording at 10 s, in its cut (22 s), after it (26 s), in its 4× range
/// (34 s), and of the second at 3 s.
fn presses() -> [Vec<Click>; 2] {
    [
        vec![press(10.0), press(22.0), press(26.0), press(34.0)],
        vec![press(3.0)],
    ]
}

fn outputs(times: &[f64]) -> Vec<OutputTime> {
    times
        .iter()
        .map(|t| OutputTime::from_secs_f64(*t))
        .collect()
}

#[test]
fn dots_stand_at_the_output_times_of_the_presses() {
    let project = project();
    let media = recordings(&project);
    let [first, second] = presses();
    let dots = click_dots(&project, |m| match m {
        m if *m == media[0] => Some(&first),
        m if *m == media[1] => Some(&second),
        _ => None,
    });
    let at: Vec<OutputTime> = dots.iter().map(|d| d.press.at).collect();
    // 26 s is 5 s of cut later; 34 s is 30 + 4 / 4 after the cut; the second recording starts
    // at 49 s (60 − 5 cut − 6 saved by 4×).
    assert_eq!(
        at,
        outputs(&[10.0, 21.0, 26.0, 52.0]),
        "the cut press is gone"
    );
    assert!(dots.iter().all(|d| !d.hidden));
}

/// The editor with a tall timeline (the «Курсор» row in sight) and the presses of the
/// recordings read.
fn with_clicks() -> H {
    let layout = crate::settings::EditorLayout {
        timeline_height: 400.0,
        ..Default::default()
    };
    let mut harness = harness_with(egui::vec2(1280.0, 1000.0), layout);
    let editor = &mut harness.state_mut().editor;
    let media = recordings(editor.doc.project());
    for (media, clicks) in media.into_iter().zip(presses()) {
        let pressed = crate::editor::presses::Pressed {
            clicks,
            keys: Vec::new(),
        };
        editor.presses.arrive(media, Ok(pressed));
    }
    editor.retrack();
    harness.run();
    harness
}

#[test]
fn cursor_tab_edits_are_cursor_and_clicks() {
    let mut harness = tall();
    cursor_tab(&mut harness);
    let own = harness.get_by_label(names::CURSOR_OWN_PICK);
    assert!(own.accesskit_node().is_disabled(), "own cursors are frozen");
    click(&mut harness, egui::Role::Switch, names::CURSOR_SHOW);
    type_in(&mut harness, names::CURSOR_SIZE, 3.0);
    click(&mut harness, egui::Role::RadioButton, names::CURSOR_EXACT);
    click(&mut harness, egui::Role::RadioButton, names::CLICK_RIPPLE);
    click(&mut harness, egui::Role::RadioButton, names::RED);
    type_in(&mut harness, names::CLICK_SIZE, 1.5);
    harness.get_by_label(text::FINE).click();
    harness.run();
    type_in(&mut harness, names::CLICK_OPACITY, 50.0);
    type_in(&mut harness, names::CLICK_DURATION, 0.8);
    type_in(&mut harness, names::CURSOR_BOUNCE_TIME, 0.2);
    click(&mut harness, egui::Role::Switch, names::CURSOR_LOOP);
    let lines = [
        "cursor --show false",
        "cursor --size 3",
        "cursor --smoothing exact",
        "clicks --style ripple",
        "clicks --color #ef4444ff",
        "clicks --scale 1.5",
        "clicks --opacity 0.5",
        "clicks --duration 0.8",
        "cursor --bounce-time 0.2",
        "cursor --loop true",
    ];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    harness.snapshot("inspector_cursor");
    undo(&mut harness);
    undo(&mut harness);
    assert_eq!(
        project_json(&harness),
        json(&cli(&lines[..8])),
        "each edit is one step"
    );
}

#[test]
fn cursor_tab_resets_are_one_step_each() {
    let mut harness = tall();
    cursor_tab(&mut harness);
    let cursor = names::reset_group(names::CURSOR_GROUP);
    let clicks = names::reset_group(names::CLICK_GROUP);
    assert!(harness.get_by_label(&cursor).accesskit_node().is_disabled());
    type_in(&mut harness, names::CURSOR_SIZE, 3.0);
    click(&mut harness, egui::Role::RadioButton, names::CLICK_ECHO);
    let edited = ["cursor --size 3", "clicks --style echo"];
    harness.get_by_label(&cursor).click();
    harness.run();
    harness.get_by_label(&clicks).click();
    harness.run();
    let mut lines = edited.to_vec();
    lines.push("cursor --show true --size 2.5 --smoothing focused");
    lines.push("clicks --style off --color #2563ebff --scale 1");
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&lines[..3])), "one step");
}

#[test]
fn a_picked_click_edits_are_click_set() {
    let mut harness = with_clicks();
    let dot = rows_text::click_dot(&timecode(OutputTime::from_secs_f64(21.0)));
    harness.get_by_label(&dot).click();
    harness.run();
    let picked = Selected::Click(OutputTime::from_secs_f64(21.0));
    assert_eq!(harness.state().editor.view.object, Some(picked));
    let head = harness.get_by_role_and_label(egui::Role::Heading, names::CLICK);
    assert_eq!(head.value().as_deref(), Some("0:21.00 · левая кнопка"));
    let reset = names::reset_group(names::CLICK_ONE);
    assert!(harness.get_by_label(&reset).accesskit_node().is_disabled());
    // Every click is off by default: a style first, then «Выкл» hides this one.
    click(&mut harness, egui::Role::RadioButton, names::CLICK_RIPPLE);
    click(&mut harness, egui::Role::RadioButton, names::CLICK_OFF);
    type_in(&mut harness, names::CLICK_SIZE, 2.0);
    let lines = [
        "click set 21 --style ripple",
        "click set 21 --style off",
        "click set 21 --scale 2",
    ];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    let hidden: Vec<bool> = harness
        .state()
        .editor
        .tracks
        .clicks
        .iter()
        .map(|d| d.hidden)
        .collect();
    assert_eq!(
        hidden,
        [false, true, false, false],
        "a hidden click is a ring"
    );
    harness.snapshot("inspector_click");
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&lines[..2])), "one step");
    harness.get_by_label(&reset).click();
    harness.run();
    assert_eq!(
        project_json(&harness),
        json(&cli(&[lines[0], lines[1], "click reset 21"]))
    );
    assert_eq!(
        harness.state().editor.view.object,
        Some(picked),
        "still picked"
    );
}

#[test]
fn a_click_near_a_dot_picks_it_and_adds_nothing() {
    let mut harness = with_clicks();
    let at = OutputTime::from_secs_f64(21.0);
    let zone = harness
        .get_by_label(&rows_text::click_dot(&timecode(at)))
        .rect();
    // 6 pt beside the dot, near the top of the row: once a miss that added a callout event.
    let near = zone.center() + egui::vec2(6.0, 2.0 - zone.height() / 2.0);
    harness.event(egui::Event::PointerMoved(near));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: near,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run();
    let editor = &harness.state().editor;
    assert_eq!(editor.view.object, Some(Selected::Click(at)));
    assert_eq!(project_json(&harness), json(&project()), "nothing added");
}
