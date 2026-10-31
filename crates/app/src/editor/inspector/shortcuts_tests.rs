//! Shortcuts of the recordings (7.16): the chips of the «Клавиши» row stand where the keys
//! effect shows its plates — through a cut, a 4× range and two recordings, repeats counted — and
//! are pale while the plate is off; the panel «Показ клавиш» counts them and shows a sample.

use dc_core::{KeyPress, KeyTracks, OutputTime, SourceTime};
use dc_ui_kit::strings::{editor as text, inspector as names};
use eframe::egui;
use egui_kittest::kittest::Queryable;

use super::tests::{H, cli, project_json};
use super::zoom_tests::click;
use crate::editor::editing_tests::json;
use crate::editor::presses::{Pressed, recordings};
use crate::editor::tests::{harness_with, project};
use crate::editor::timeline::{Region, Row, Sample, Tracks};

fn key(at: f64, name: &str) -> KeyPress {
    KeyPress {
        at: SourceTime::from_secs_f64(at),
        name: name.to_owned(),
    }
}

/// Shortcuts of the first recording at 10 s, in its cut (22 s), twice after it (26, 26.5 s), in
/// its 4× range (34 s); of the second at 3 s.
fn shortcuts() -> [Vec<KeyPress>; 2] {
    [
        vec![
            key(10.0, "Ctrl+S"),
            key(22.0, "F5"),
            key(26.0, "Ctrl+Shift+P"),
            key(26.5, "Ctrl+Shift+P"),
            key(34.0, "F5"),
        ],
        vec![key(3.0, "Ctrl+S")],
    ]
}

fn chips(tracks: &Tracks) -> Vec<Region> {
    let row = tracks.rows.iter().find(|t| t.row == Row::Keys);
    row.map(|t| t.regions.clone()).unwrap_or_default()
}

fn spans(regions: &[Region]) -> Vec<(f64, f64, String)> {
    regions
        .iter()
        .map(|r| {
            let at = |t: OutputTime| (t.as_secs_f64() * 1000.0).round() / 1000.0;
            (at(r.range.start()), at(r.range.end()), r.label.clone())
        })
        .collect()
}

#[test]
fn chips_stand_where_the_plates_show() {
    let project = project();
    let media = recordings(&project);
    let keys: KeyTracks = media
        .iter()
        .map(|m| m.path.clone())
        .zip(shortcuts())
        .collect();
    let mut tracks = Tracks::of(&project, &[], Default::default());
    tracks.place_keys(&project, &keys);
    // 26 s is 5 s of cut later; 34 s is 30 + 4 / 4 after the cut; the second recording starts
    // at 49 s. A plate stays 1.5 s after its last press.
    let expected = [
        (10.0, 11.5, "Ctrl+S"),
        (21.0, 23.0, "Ctrl+Shift+P ×2"),
        (26.0, 27.5, "F5"),
        (52.0, 53.5, "Ctrl+S"),
    ];
    let expected: Vec<(f64, f64, String)> = expected
        .iter()
        .map(|(a, b, n)| (*a, *b, (*n).to_owned()))
        .collect();
    let placed = chips(&tracks);
    assert_eq!(spans(&placed), expected, "the cut press is gone");
    assert!(placed.iter().all(|r| r.faded && r.item.is_none()));
    assert_eq!(tracks.shortcuts.names, 3);
    let sample = Sample {
        name: "Ctrl+Shift+P".to_owned(),
        count: 2,
    };
    assert_eq!(tracks.shortcuts.sample, Some(sample));
}

/// The editor with a tall timeline (the «Клавиши» row in sight) and the shortcuts read.
fn with_shortcuts() -> H {
    let layout = crate::settings::EditorLayout {
        timeline_height: 400.0,
        ..Default::default()
    };
    let mut harness = harness_with(egui::vec2(1280.0, 1000.0), layout);
    let editor = &mut harness.state_mut().editor;
    let media = recordings(editor.doc.project());
    for (media, keys) in media.into_iter().zip(shortcuts()) {
        let pressed = Pressed {
            clicks: Vec::new(),
            keys,
        };
        editor.presses.arrive(media, Ok(pressed));
    }
    editor.retrack();
    harness.run();
    harness
}

#[test]
fn the_keys_panel_counts_the_shortcuts_and_shows_a_sample() {
    let mut harness = with_shortcuts();
    let head = harness.get_by_role_and_label(egui::Role::Heading, text::KEYS);
    assert_eq!(head.value().as_deref(), Some("3 сочетания в записях"));
    assert!(harness.query_by_label("Ctrl + Shift + P ×2").is_some());
    harness.snapshot("editor_key_chips_off");
    click(&mut harness, egui::Role::Switch, names::KEYS_SHOW);
    assert_eq!(project_json(&harness), json(&cli(&["keys --show true"])));
    let placed = chips(&harness.state().editor.tracks);
    assert_eq!(placed.len(), 4);
    assert!(placed.iter().all(|r| !r.faded), "the plate is on");
    harness.snapshot("editor_key_chips");
}

#[test]
fn plate_samples_read_like_on_the_mockup() {
    assert_eq!(
        names::plate_sample("Ctrl+Shift+P", 2),
        "Ctrl + Shift + P ×2"
    );
    assert_eq!(names::plate_sample("F5", 1), "F5");
    assert_eq!(names::plate_sample("Ctrl++", 1), "Ctrl + +");
    assert_eq!(names::shortcuts_found(1), "1 сочетание в записях");
    assert_eq!(names::shortcuts_found(0), "в записях нет сочетаний");
}
