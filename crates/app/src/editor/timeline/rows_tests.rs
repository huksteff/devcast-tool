//! Rows of the timeline (7.10): «+ Дорожка ▾» and «+ Аудиодорожка», × with and without the
//! question, the layer order by dragging an annotation header, the sound button, the ghost of
//! quick adding, Z, T and Tab. Each change gives the project `devcast` gives for the same
//! commands, in one undo step.

use dc_core::{OutputTime, TimeDelta};
use dc_ui_kit::strings::timeline as text;
use eframe::egui::{Key, Modifiers, Pos2, pos2};
use egui_kittest::kittest::Queryable;

use super::clips_tests::{H, move_to, press, project_json, started, undo_is_one_step};
use super::regions_tests::{annotation, at, by_cli, click, drag, on_row, picked, words, zoom};
use super::tracks::{Row, Tracks};
use crate::editor::adding::slot;
use crate::editor::editing_tests::json;
use crate::editor::selection::Item;
use crate::editor::test_project::{output, project};

/// The editor with a timeline tall enough for every row.
fn tall() -> H {
    let layout = crate::settings::EditorLayout {
        timeline_height: 560.0,
        ..Default::default()
    };
    let mut harness =
        crate::editor::tests::harness_with(eframe::egui::vec2(1280.0, 1000.0), layout);
    harness.run();
    harness
}

fn row_names(harness: &H) -> Vec<Row> {
    let rows = &harness.state().editor.tracks.rows;
    rows.iter().map(|t| t.row).collect()
}

/// The button at the right of the header of the row named `name`.
fn head_button(harness: &H, name: &str) -> Pos2 {
    let head = harness.get_by_label(name).rect();
    pos2(head.right() - 4.0 - 11.0, head.center().y)
}

fn expect(harness: &H, lines: &[&str]) {
    let lines: Vec<Vec<String>> = lines.iter().map(|l| words(l)).collect();
    assert_eq!(project_json(harness), json(&by_cli(&lines)));
}

/// The start of the region of `item` now, as `devcast` takes it.
fn start_of(harness: &H, item: Item) -> String {
    let rows = &harness.state().editor.tracks.rows;
    let region = rows.iter().flat_map(|t| &t.regions);
    let mut found = region
        .filter(|r| r.item == Some(item))
        .map(|r| r.range.start());
    crate::editor::montage::arg(found.next_back().unwrap())
}

#[test]
fn the_menu_and_the_bottom_button_add_empty_rows() {
    let mut harness = tall();
    harness.get_by_label(text::ADD_TRACK).click();
    harness.run();
    harness.snapshot("editor_track_menu");
    harness.get_by_label(text::ADD_ANNOTATIONS_ROW).click();
    harness.run();
    assert!(row_names(&harness).contains(&Row::Annotations(2)));
    harness.get_by_label(text::ADD_AUDIO).click();
    harness.run();
    assert!(row_names(&harness).contains(&Row::Audio(1)));
    assert_eq!(
        project_json(&harness),
        json(&project()),
        "the project has no empty rows"
    );
    let spot = head_button(&harness, "Аннотации 3");
    click(&mut harness, spot, Modifiers::NONE);
    assert!(
        !row_names(&harness).contains(&Row::Annotations(2)),
        "empty: no question"
    );
    assert!(harness.query_by_label(text::REMOVE_CONFIRM).is_none());
}

#[test]
fn removing_a_row_with_annotations_asks_first() {
    let mut harness = started();
    let spot = head_button(&harness, "Аннотации 1");
    click(&mut harness, spot, Modifiers::NONE);
    harness.snapshot("editor_remove_row");
    harness.get_by_label(text::REMOVE_CANCEL).click();
    harness.run();
    assert_eq!(project_json(&harness), json(&project()));
    assert!(harness.query_by_label(text::REMOVE_CONFIRM).is_none());
    let spot = head_button(&harness, "Аннотации 1");
    click(&mut harness, spot, Modifiers::NONE);
    harness.get_by_label(text::REMOVE_CONFIRM).click();
    harness.run();
    expect(
        &harness,
        &["annot rm a1", "annot rm a2", "annot move a3 --track 0"],
    );
    assert!(!row_names(&harness).contains(&Row::Annotations(1)));
    undo_is_one_step(&mut harness);
}

#[test]
fn removing_an_extra_sound_row_removes_its_files() {
    let mut harness = tall();
    let spot = head_button(&harness, "Аудио 1");
    click(&mut harness, spot, Modifiers::NONE);
    harness.get_by_label(text::REMOVE_CONFIRM).click();
    harness.run();
    expect(&harness, &["audio rm au1"]);
    assert!(!row_names(&harness).contains(&Row::Audio(0)));
    undo_is_one_step(&mut harness);
}

#[test]
fn dragging_an_annotation_header_changes_the_layer_order() {
    let mut harness = started();
    let from = harness.get_by_label("Аннотации 2").rect().center();
    let to = harness.get_by_label("Аннотации 1").rect().center();
    drag(&mut harness, from, to, Modifiers::NONE);
    expect(
        &harness,
        &[
            "annot move a1 --track 1",
            "annot move a2 --track 1",
            "annot move a3 --track 0",
        ],
    );
    undo_is_one_step(&mut harness);
}

#[test]
fn the_sound_button_mutes_a_recording_track() {
    let mut harness = tall();
    let spot = head_button(&harness, text::MIC);
    click(&mut harness, spot, Modifiers::NONE);
    expect(&harness, &["audio channel mic --mute true"]);
    undo_is_one_step(&mut harness);
}

#[test]
fn the_ghost_adds_a_zoom_where_the_pointer_is() {
    let mut harness = started();
    let spot = on_row(&harness, text::ZOOMS, 1.5);
    move_to(&mut harness, spot);
    harness.snapshot("editor_ghost");
    click(&mut harness, spot, Modifiers::NONE);
    assert_eq!(picked(&harness), [zoom(4)]);
    let start = start_of(&harness, zoom(4));
    expect(&harness, &[&format!("zoom add --at {start} --duration 1")]);
    undo_is_one_step(&mut harness);
}

/// The item `name` of the menu of kinds the ghost opened at `spot` (regions of that kind are
/// named so too: the one starting nearest right of `spot`; the menu may stand above it).
fn kind_item(harness: &H, spot: Pos2, name: &str) -> Pos2 {
    let rects: Vec<eframe::egui::Rect> = harness
        .query_all_by_label(name)
        .map(|n| n.rect())
        .filter(|r| r.min.x >= spot.x - 1.0)
        .collect();
    let nearest = rects
        .into_iter()
        .min_by(|a, b| (a.min.x - spot.x).total_cmp(&(b.min.x - spot.x)));
    nearest.unwrap().center()
}

#[test]
fn the_ghost_asks_the_kind_of_a_new_annotation() {
    let mut harness = started();
    let spot = on_row(&harness, "Аннотации 2", 60.0);
    click(&mut harness, spot, Modifiers::NONE);
    assert!(
        harness.state().editor.view.kinds.is_some(),
        "the menu opens"
    );
    expect(&harness, &[]);
    harness.snapshot("editor_kind_menu");
    let shape = kind_item(&harness, spot, text::SHAPE);
    click(&mut harness, shape, Modifiers::NONE);
    assert!(harness.state().editor.view.kinds.is_none());
    assert_eq!(picked(&harness), [annotation(4)]);
    let start = start_of(&harness, annotation(4));
    expect(
        &harness,
        &[&format!(
            "annot add shape --at {start} --duration 1 --track 1"
        )],
    );
    undo_is_one_step(&mut harness);
}

#[test]
fn a_drawing_from_the_menu_is_picked_under_the_brush_bar() {
    let mut harness = started();
    let spot = on_row(&harness, "Аннотации 2", 60.0);
    click(&mut harness, spot, Modifiers::NONE);
    let drawing = kind_item(&harness, spot, text::DRAWING);
    click(&mut harness, drawing, Modifiers::NONE);
    assert_eq!(picked(&harness), [annotation(4)]);
    let start = start_of(&harness, annotation(4));
    expect(
        &harness,
        &[&format!(
            "annot add drawing --at {start} --duration 1 --track 1"
        )],
    );
    let pencil = dc_ui_kit::strings::annotations::BRUSH_PENCIL;
    assert!(harness.query_by_label(pencil).is_some(), "the brush bar");
}

#[test]
fn esc_closes_the_menu_of_kinds() {
    let mut harness = started();
    let spot = on_row(&harness, "Аннотации 2", 60.0);
    click(&mut harness, spot, Modifiers::NONE);
    press(&mut harness, Key::Escape, Modifiers::NONE);
    assert!(harness.state().editor.view.kinds.is_none());
    expect(&harness, &[]);
}

#[test]
fn the_ghost_fits_the_room_it_is_in() {
    let tracks = Tracks::of(&project(), &[], Default::default());
    let t = OutputTime::from_secs_f64;
    assert_eq!(
        slot(&tracks, Row::Zooms, t(4.5)),
        Some(output(4.0, 5.0)),
        "before a zoom"
    );
    assert_eq!(slot(&tracks, Row::Zooms, t(6.0)), None, "on a zoom");
    assert_eq!(
        slot(&tracks, Row::Zooms, t(12.5)),
        Some(output(12.0, 13.0)),
        "1 s gap"
    );
    let annotations = slot(&tracks, Row::Annotations(1), t(48.7));
    assert_eq!(annotations, Some(output(48.0, 49.0)), "inside its clip");
    assert_eq!(
        slot(&tracks, Row::Cursor, t(104.8)),
        Some(output(104.0, 105.0))
    );
    let room = slot(&tracks, Row::Audio(1), t(30.0)).map(|r| r.duration());
    assert_eq!(room, Some(TimeDelta::from_secs_f64(1.0)));
}

#[test]
fn z_adds_a_zoom_at_the_playhead() {
    let mut harness = started();
    press(&mut harness, Key::Z, Modifiers::NONE);
    expect(&harness, &["zoom add --at 33.400000 --duration 1"]);
    assert_eq!(picked(&harness), [zoom(4)]);
    undo_is_one_step(&mut harness);
}

#[test]
fn t_adds_an_annotation_on_the_lowest_free_row_and_tab_walks_them() {
    let mut harness = started();
    at(&mut harness, 25.0);
    press(&mut harness, Key::T, Modifiers::NONE);
    expect(
        &harness,
        &["annot add text --at 25.000000 --duration 1 --track 1"],
    );
    assert_eq!(picked(&harness), [annotation(4)]);
    press(&mut harness, Key::Tab, Modifiers::NONE);
    assert_eq!(
        picked(&harness),
        [annotation(1)],
        "after the top one, the bottom one"
    );
    press(&mut harness, Key::Tab, Modifiers::NONE);
    assert_eq!(picked(&harness), [annotation(4)]);
    undo_is_one_step(&mut harness);
}

/// A WAV file of `secs` seconds of silence, 48 kHz mono 16 bit.
fn wav(path: &std::path::Path, secs: u32) {
    let data = 48_000 * 2 * secs;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    for (value, size) in [
        (16, 4),
        (1, 2),
        (1, 2),
        (48_000, 4),
        (96_000, 4),
        (2, 2),
        (16, 2),
    ] {
        bytes.extend_from_slice(&u32::to_le_bytes(value)[..size]);
    }
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    bytes.resize(bytes.len() + data as usize, 0);
    std::fs::write(path, bytes).unwrap();
}

fn add_sound(harness: &mut H, path: &std::path::Path, track: u32) {
    let file = crate::editor::adding::SoundFile {
        path: path.to_owned(),
        at: OutputTime::from_secs_f64(30.0),
        track,
        room: TimeDelta::from_secs_f64(1.5),
    };
    harness
        .state_mut()
        .editor
        .ask(crate::editor::Action::Sound(file));
    harness.run();
}

#[test]
fn a_sound_file_is_copied_onto_its_row_in_one_step() {
    let mut harness = started();
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("тон 2.wav");
    wav(&source, 2);
    add_sound(&mut harness, &source, 1);
    let editor = &harness.state().editor;
    let state = editor
        .doc
        .project()
        .state::<dc_fx_audio_tracks::Audio>()
        .unwrap();
    let clip = state.clips.last().unwrap();
    assert_eq!(
        (clip.track, clip.range),
        (1, output(30.0, 31.5)),
        "cut to the room"
    );
    assert_eq!(clip.file.path, "assets/audio/тон 2.wav");
    assert!(
        editor
            .doc
            .dir()
            .root()
            .join("assets/audio/тон 2.wav")
            .is_file()
    );
    assert_eq!(picked(&harness), [Item::of(clip.id)]);
    undo_is_one_step(&mut harness);
}

#[test]
fn a_refused_sound_file_leaves_no_copy() {
    let mut harness = started();
    harness.state_mut().errors = Some(Vec::new());
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("тон.wav");
    wav(&source, 2);
    add_sound(&mut harness, &source, 0);
    assert_eq!(
        harness.state().errors.as_ref().map(Vec::len),
        Some(1),
        "a toast"
    );
    assert_eq!(project_json(&harness), json(&project()));
    let root = harness.state().editor.doc.dir().root().to_owned();
    assert!(!root.join("assets/audio/тон.wav").exists());
}
