use dc_core::project::{Media, MediaKind, SpeedRange};
use dc_core::time::Source;
use dc_core::{
    Clip, ClipAudio, ClipId, Id, OutputTime, Project, Range, Rect, SourceTime, TimeDelta,
};
use dc_fx_annotations::{Annotation, Annotations, AnnotationsState, Kind};
use dc_fx_audio_tracks::{Audio, AudioClip, AudioState, SoundFile};
use dc_fx_callout::{Action, Callout, CalloutEvent, CalloutState};
use dc_fx_captions::{CaptionsEffect, CaptionsState, Cue, Word};
use dc_fx_crop::{Crop, CropState};
use dc_fx_zoom::{Mode, Zoom, ZoomEffect, ZoomState};
use dc_ui_kit::strings::{editor as text, timeline as names};
use egui_kittest::kittest::{NodeT as _, Queryable};

use super::view::{Panel, PreviewScale, Quality};
use super::*;
use timeline::{Row, Track};

fn source(start: f64, end: f64) -> Range<Source> {
    Range::new(
        SourceTime::from_secs_f64(start),
        SourceTime::from_secs_f64(end),
    )
    .unwrap()
}

fn output(start: f64, end: f64) -> Range<dc_core::time::Output> {
    Range::new(
        OutputTime::from_secs_f64(start),
        OutputTime::from_secs_f64(end),
    )
    .unwrap()
}

fn clip(number: u32, path: &str, kind: MediaKind, secs: f64, size: (u32, u32)) -> Clip {
    Clip {
        id: ClipId::from_number(number),
        media: Media {
            path: path.to_owned(),
            kind,
            duration: TimeDelta::from_secs_f64(secs),
            size: dc_core::PixelSize {
                width: size.0,
                height: size.1,
            },
        },
        trim: source(0.0, secs),
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop: Rect::FULL,
        audio: ClipAudio::default(),
    }
}

const SCREEN: &str = "media/screen.mp4";
const SCREEN_2: &str = "media/screen-2.mp4";

/// Three clips (a cut and a 4× range in the first, an imported video last) with zooms, captions,
/// annotations on two tracks, callout events and music; system sound muted.
pub(super) fn project() -> Project {
    let mut first = clip(1, SCREEN, MediaKind::Recording, 60.0, (2560, 1440));
    first.cuts = vec![source(20.0, 25.0)];
    first.speeds = vec![SpeedRange {
        range: source(30.0, 38.0),
        factor: 4.0,
    }];
    let mut project = Project {
        clips: vec![
            first,
            clip(2, SCREEN_2, MediaKind::Recording, 38.0, (1920, 1080)),
            clip(
                3,
                "media/логотип.mp4",
                MediaKind::Imported,
                18.0,
                (1440, 1080),
            ),
        ],
        ..Project::default()
    };
    let ids: Vec<ClipId> = project.clips.iter().map(|c| c.id).collect();
    project.clip_ids.ensure_above(ids);
    let zoom = |n, media: &str, range, scale, mode| Zoom {
        id: Id::from_number(n),
        media: media.to_owned(),
        range,
        scale,
        focus: dc_core::Point { x: 0.5, y: 0.5 },
        mode,
    };
    let zooms = ZoomState {
        zooms: vec![
            zoom(1, SCREEN, source(5.0, 12.0), 1.5, Mode::Follow),
            zoom(2, SCREEN, source(13.0, 17.0), 2.0, Mode::Follow),
            zoom(3, SCREEN_2, source(2.0, 9.0), 1.8, Mode::Fixed),
        ],
        ..ZoomState::default()
    };
    project.set_state::<ZoomEffect>(&zooms).unwrap();
    project.set_state::<CaptionsEffect>(&captions()).unwrap();
    project.set_state::<Annotations>(&annotations()).unwrap();
    project.set_state::<Callout>(&callouts()).unwrap();
    project.set_state::<Audio>(&audio()).unwrap();
    let mut crop = CropState::default();
    crop.settings.aspect = dc_fx_crop::Aspect::Wide16x9;
    project.set_state::<Crop>(&crop).unwrap();
    project
}

fn captions() -> CaptionsState {
    let cue = |n, media: &str, start: f64, end: f64, said: &str| Cue {
        id: Id::from_number(n),
        media: media.to_owned(),
        range: source(start, end),
        words: vec![Word {
            text: said.to_owned(),
            range: source(start, end),
        }],
    };
    CaptionsState {
        cues: vec![
            cue(1, SCREEN, 1.0, 8.0, "Сегодня покажу вход"),
            cue(2, SCREEN, 9.0, 18.0, "Открываем страницу логина"),
            cue(3, SCREEN_2, 5.0, 12.0, "Теперь настройки профиля"),
        ],
        ..CaptionsState::default()
    }
}

fn annotations() -> AnnotationsState {
    let annotation = |n, kind, media: &str, range, track| Annotation {
        id: Id::from_number(n),
        kind,
        media: media.to_owned(),
        range,
        track,
        rect: Rect::new(0.1, 0.1, 0.3, 0.1).unwrap(),
        rotation: 0.0,
        enter: Default::default(),
        exit: Default::default(),
        body: serde_json::Map::new(),
        keys: Vec::new(),
    };
    AnnotationsState {
        annotations: vec![
            annotation(1, Kind::Text, SCREEN, source(28.0, 40.0), 0),
            annotation(2, Kind::Blur, SCREEN_2, source(0.0, 15.0), 0),
            annotation(3, Kind::Shape, SCREEN, source(10.0, 14.0), 1),
        ],
        ..AnnotationsState::default()
    }
}

fn callouts() -> CalloutState {
    let event = |n, at: f64, secs: f64, action| CalloutEvent {
        id: Id::from_number(n),
        media: SCREEN.to_owned(),
        at: SourceTime::from_secs_f64(at),
        duration: TimeDelta::from_secs_f64(secs),
        action,
    };
    let said = "«Сохраните»".to_owned();
    CalloutState {
        events: vec![
            event(1, 45.0, 1.5, Action::On),
            event(2, 46.5, 5.0, Action::Text { text: said }),
            event(3, 52.0, 1.5, Action::Off),
        ],
        ..CalloutState::default()
    }
}

fn audio() -> AudioState {
    let mut state = AudioState::default();
    state.clips.push(AudioClip {
        id: Id::from_number(1),
        file: SoundFile {
            path: "assets/audio/музыка_фон.mp3".to_owned(),
            duration: TimeDelta::from_secs_f64(240.0),
        },
        track: 0,
        range: output(0.0, 105.0),
        offset: TimeDelta::ZERO,
        volume: 30.0,
        gain: 1.0,
        muted: false,
        normalize: false,
    });
    let muted = dc_core::ChannelLevels {
        muted: Some(true),
        ..Default::default()
    };
    state.settings.channels.insert("system".to_owned(), muted);
    state
}

/// The editor of [`project`] saved in a temporary `Демо-запись.dcproj` with unsaved changes (a
/// `session.json`); the folder lives while the returned guard does.
pub(super) fn editor() -> (tempfile::TempDir, EditorWindow) {
    let temp = tempfile::tempdir().unwrap();
    let dir = ProjectDir::create(temp.path().join("Демо-запись.dcproj")).unwrap();
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
    outcome: Option<Outcome>,
    /// Some frame asked for the settings window, on this page.
    pub settings: Option<crate::settings_window::Page>,
    /// The shortcuts the settings give the editor.
    pub keys: crate::keymap::EditorKeys,
}

pub(super) fn harness(size: egui::Vec2) -> egui_kittest::Harness<'static, State> {
    let (temp, editor) = editor();
    let state = State {
        _temp: temp,
        editor,
        outcome: None,
        settings: None,
        keys: crate::keymap::EditorKeys::default(),
    };
    dc_ui_kit::testing::harness()
        .with_size(size)
        .build_ui_state(
            |ui, state: &mut State| {
                let now = std::time::Instant::now();
                let outcome = editor_ui(ui, &mut state.editor, 300.0, &state.keys, now);
                assert!(outcome.errors.is_empty(), "{:?}", outcome.errors);
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
fn editor_window() {
    let mut harness = harness(SIZE);
    harness.run();
    harness.snapshot("editor");
    let outcome = harness.state().outcome.clone().unwrap();
    assert!((outcome.timeline_height - 300.0).abs() < 1.0, "{outcome:?}");
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
    harness
        .get_by_role_and_label(egui::Role::Button, text::CAPTIONS)
        .click();
    harness.get_by_label(text::QUALITY_FAST).click();
    harness.get_by_label(text::ACTUAL).click();
    harness.get_by_label(text::BLADE).click();
    harness.get_by_label(text::FINE).click();
    harness.run();
    let view = &harness.state().editor.view;
    assert_eq!(view.panel, Panel::Captions);
    assert_eq!(view.quality, Quality::Fast);
    assert_eq!(view.scale, PreviewScale::Actual);
    assert_eq!(view.tool, super::view::Tool::Blade);
    assert!(view.fine_open);
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
    for label in [text::EXPORT, text::PLAY, names::SUGGEST_ZOOMS] {
        assert!(
            harness.get_by_label(label).accesskit_node().is_disabled(),
            "{label} must wait for its task"
        );
    }
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
    let speeds: Vec<_> = tracks.speeds.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(speeds, ["4×"]);
    let seams: Vec<f64> = tracks.seams.iter().map(|t| t.as_secs_f64()).collect();
    assert_eq!(seams, [20.0, 25.0, 27.0]);
}
