use dc_core::{OutputTime, TimeDelta};
use dc_project::ProjectDir;

pub(super) use super::test_project::project;
use dc_ui_kit::strings::{editor as text, timeline as names};
use egui_kittest::kittest::{NodeT as _, Queryable};

use super::view::{PreviewScale, Quality};
use super::*;
use timeline::{Row, Track};

/// The editor of [`project`] saved in a temporary `Демо-запись.devcast` with unsaved changes (a
/// `session.json`); the folder lives while the returned guard does.
pub(super) fn editor() -> (tempfile::TempDir, EditorWindow) {
    let temp = tempfile::tempdir().unwrap();
    let dir = ProjectDir::create(temp.path().join("Демо-запись.devcast")).unwrap();
    dir.save(&project()).unwrap();
    dir.save_session(&project()).unwrap();
    let sound = vec!["system".to_owned(), "mic".to_owned()];
    let (mut editor, problems) = EditorWindow::open(dir, sound).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    editor.view.playhead = OutputTime::from_secs_f64(33.4);
    (temp, editor)
}

pub(super) struct State {
    _temp: tempfile::TempDir,
    pub editor: EditorWindow,
    pub outcome: Option<Outcome>,
    /// The timeline sizes the settings give the editor.
    pub layout: crate::settings::EditorLayout,
    /// Some frame asked for the settings window, on this page.
    pub settings: Option<crate::settings_window::Page>,
    /// The shortcuts the settings give the editor.
    pub keys: crate::keymap::EditorKeys,
    /// The inspector groups of the settings: a frame that toggles one stores it here.
    pub groups: crate::settings::InspectorGroups,
    /// The moment of the frames, when a test drives the clock; otherwise now.
    pub now: Option<std::time::Instant>,
    /// The errors of the frames (toasts) when a test expects some; otherwise an error fails it.
    pub errors: Option<Vec<String>>,
    /// Exports the frames told done (toasts): `run` draws several frames, and only the first
    /// one after a job ends tells it.
    pub done: Vec<String>,
}

pub(super) fn harness(size: egui::Vec2) -> egui_kittest::Harness<'static, State> {
    harness_with(size, crate::settings::EditorLayout::default())
}

/// [`harness`] with the timeline sizes `layout` of the settings from the first frame.
pub(super) fn harness_with(
    size: egui::Vec2,
    layout: crate::settings::EditorLayout,
) -> egui_kittest::Harness<'static, State> {
    let (temp, editor) = editor();
    harness_of(temp, editor, size, layout)
}

/// The harness of `editor`, whose project lives in `temp`.
pub(super) fn harness_of(
    temp: tempfile::TempDir,
    editor: EditorWindow,
    size: egui::Vec2,
    layout: crate::settings::EditorLayout,
) -> egui_kittest::Harness<'static, State> {
    let state = State {
        _temp: temp,
        editor,
        outcome: None,
        layout,
        settings: None,
        keys: crate::keymap::EditorKeys::default(),
        groups: crate::settings::InspectorGroups::default(),
        now: None,
        errors: None,
        done: Vec::new(),
    };
    dc_ui_kit::testing::harness()
        .with_size(size)
        .build_ui_state(
            |ui, state: &mut State| {
                let now = state.now.unwrap_or_else(std::time::Instant::now);
                let prefs = (&state.keys, &state.groups);
                let outcome = editor_ui(ui, &mut state.editor, state.layout, prefs, now);
                if let Some(groups) = outcome.groups.clone() {
                    state.groups = groups;
                }
                match state.errors.as_mut() {
                    Some(errors) => errors.extend(outcome.errors.iter().cloned()),
                    None => assert!(outcome.errors.is_empty(), "{:?}", outcome.errors),
                }
                state.done.extend(outcome.done.iter().cloned());
                state.settings = state.settings.or(outcome.open_settings);
                state.outcome = Some(outcome);
            },
            state,
        )
}

#[test]
fn opens_in_the_middle_of_the_hud_monitor() {
    let left = dc_capture::Area::new(-1920, 11, 1920, 1080).unwrap();
    assert_eq!(
        crate::monitor::centred(left, 1.0, SIZE),
        egui::pos2(-1600.0, 151.0)
    );
}

#[test]
fn the_timeline_takes_its_height_back_after_the_window_was_small() {
    let top = |harness: &egui_kittest::Harness<'static, State>| {
        harness
            .get_by_label(dc_ui_kit::strings::timeline::ADD_TRACK)
            .rect()
            .top()
    };
    let mut harness = harness(SIZE);
    harness.run();
    let before = top(&harness);
    harness.set_size(egui::vec2(SIZE.x, 360.0));
    harness.run();
    assert!(top(&harness) < before, "squeezed by the small window");
    harness.set_size(SIZE);
    harness.run();
    assert!((top(&harness) - before).abs() < 1.0, "back with the room");
    let layout = harness.state().outcome.clone().unwrap().layout;
    assert!(
        (layout.timeline_height - 300.0).abs() < 1.0,
        "the remembered height did not change: {layout:?}"
    );
}

#[test]
fn editor_window() {
    let mut harness = harness(SIZE);
    harness.run();
    harness.snapshot("editor");
    let outcome = harness.state().outcome.clone().unwrap();
    assert!(
        (outcome.layout.timeline_height - 300.0).abs() < 1.0,
        "{outcome:?}"
    );
    harness.get_by_label("Демо-запись");
    harness.get_by_label(&names::annotations(2));
    harness.get_by_label(&names::audio(1));
    harness.get_by_label(names::SYSTEM);
}

#[test]
fn editor_window_at_its_least_size() {
    let mut harness = harness(MIN_SIZE);
    harness.run();
    harness.snapshot("editor_min");
}

#[test]
fn controls_hold_their_state() {
    let mut harness = harness(SIZE);
    harness.run();
    harness.get_by_label(text::QUALITY_FAST).click();
    harness.get_by_label(text::ACTUAL).click();
    harness.get_by_label(text::BLADE).click();
    harness.run();
    let view = &harness.state().editor.view;
    assert_eq!(view.quality, Quality::Fast);
    assert_eq!(view.scale, PreviewScale::Actual);
    assert_eq!(view.tool, super::view::Tool::Blade);
    assert_eq!(harness.state().settings, None);
    harness.get_by_label(text::SETTINGS).click();
    harness.run();
    assert_eq!(
        harness.state().settings,
        Some(crate::settings_window::Page::Recording)
    );
}

#[test]
fn commands_without_a_registry_command_are_disabled() {
    let mut harness = harness(SIZE);
    harness.run();
    let export = harness.get_by_label(text::EXPORT);
    assert!(
        export.accesskit_node().is_disabled(),
        "export waits for its task"
    );
}

fn ranges(track: &Track) -> Vec<(f64, f64)> {
    let secs = |t: OutputTime| (t.as_secs_f64() * 1000.0).round() / 1000.0;
    track
        .regions
        .iter()
        .map(|r| (secs(r.range.start()), secs(r.range.end())))
        .collect()
}

#[test]
fn tracks_stand_at_output_times() {
    let tracks = editor().1.tracks;
    assert!(tracks.problems.is_empty(), "{:?}", tracks.problems);
    assert_eq!(tracks.duration, TimeDelta::from_secs_f64(105.0));
    let rows: Vec<Row> = tracks.rows.iter().map(|t| t.row).collect();
    assert_eq!(
        rows,
        [
            Row::Captions,
            Row::Clips,
            Row::Zooms,
            Row::Cursor,
            Row::Keys,
            Row::Annotations(0),
            Row::Annotations(1),
            Row::Mic,
            Row::System,
            Row::Audio(0),
        ]
    );
    let row = |row: Row| tracks.rows.iter().find(|t| t.row == row).unwrap();
    // 20–25 s of the first clip is cut, 30–38 s plays 4× (2 s of output).
    assert_eq!(
        ranges(row(Row::Clips)),
        [(0.0, 49.0), (49.0, 87.0), (87.0, 105.0)]
    );
    assert_eq!(
        ranges(row(Row::Zooms)),
        [(5.0, 12.0), (13.0, 17.0), (51.0, 58.0)]
    );
    assert_eq!(row(Row::Zooms).regions[2].label, "1,8× · точка");
    assert_eq!(
        ranges(row(Row::Annotations(0))),
        [(23.0, 29.0), (49.0, 64.0)]
    );
    assert_eq!(
        ranges(row(Row::Cursor)),
        [(34.0, 35.5), (35.5, 40.5), (41.0, 42.5)]
    );
    assert_eq!(
        ranges(row(Row::Mic)),
        [(0.0, 49.0), (49.0, 87.0)],
        "recordings only"
    );
    assert!(row(Row::System).off && row(Row::System).regions.iter().all(|r| r.faded));
    assert!(row(Row::Keys).off, "keys are not shown by default");
    let speeds: Vec<_> = tracks
        .speeds
        .iter()
        .map(|s| s.region.label.as_str())
        .collect();
    assert_eq!(speeds, ["4×"]);
    let seams: Vec<f64> = tracks.seams.iter().map(|t| t.as_secs_f64()).collect();
    assert_eq!(seams, [20.0, 25.0, 27.0]);
}
