//! The sound panels on their own, as the editor places them: the tracks come from the facts
//! ([`Sources`]), edits are `audio channel`, `audio mute` and `audio set`. The editor's snapshots
//! of them and the path to the same project as `devcast` are in dc-app
//! (`inspector/sound_tests.rs`).

use std::collections::BTreeMap;

use dc_core::{ClipId, Command, OutputTime, Project, Step, TimeDelta};
use dc_ui_kit::egui;
use dc_ui_kit::panel::{Cx, Groups};
use dc_ui_kit::strings::sound as text;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use serde_json::{Value, json};

use crate::panel::{self, Sources};
use crate::tests::project;
use crate::{AudioAdd, AudioId, SoundFile};

/// Which panel the harness draws.
#[derive(Clone, Copy)]
enum Shows {
    Tab,
    Clip(u32),
    Extra(u32),
}

struct State {
    project: Project,
    defaults: Project,
    groups: BTreeMap<String, bool>,
    shows: Shows,
    recorded: Vec<String>,
    streams: Option<Vec<String>>,
    edits: Vec<Vec<Step>>,
}

fn harness(project: Project, shows: Shows, streams: Option<&[&str]>) -> Harness<'static, State> {
    let names = |all: &[&str]| all.iter().map(|s| (*s).to_owned()).collect();
    let state = State {
        project,
        defaults: Project::default(),
        groups: BTreeMap::new(),
        shows,
        recorded: names(&["mic", "system"]),
        streams: streams.map(names),
        edits: Vec::new(),
    };
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(340.0, 900.0))
        .build_ui_state(draw, state);
    harness.run();
    harness
}

fn draw(ui: &mut egui::Ui, state: &mut State) {
    let mut cx = Cx {
        project: &state.project,
        defaults: &state.defaults,
        playhead: OutputTime::ZERO,
        auto_keys: false,
        fonts: &[],
        groups: Groups::new(&mut state.groups),
        edits: Vec::new(),
        adds: Vec::new(),
        requests: Vec::new(),
    };
    let sources = Sources {
        recorded: &state.recorded,
        streams: state.streams.as_deref(),
        end: TimeDelta::from_secs_f64(24.0),
    };
    match state.shows {
        Shows::Tab => panel::ui(ui, &mut cx, &sources),
        Shows::Clip(n) => panel::clip(ui, &mut cx, ClipId::from_number(n), &sources),
        Shows::Extra(n) => panel::extra(ui, &mut cx, AudioId::from_number(n), &sources),
    }
    let edits = cx.edits;
    state.edits.extend(edits);
}

fn step(op: &str, args: Value) -> Vec<Step> {
    let Value::Object(args) = args else {
        panic!("arguments are an object");
    };
    vec![Step::new(op, args)]
}

fn click(harness: &mut Harness<'_, State>, role: egui::Role, label: &str) {
    harness.get_by_role_and_label(role, label).click();
    harness.run();
}

#[test]
fn the_tab_shows_the_recorded_tracks_and_edits_audio_channel() {
    let mut harness = harness(project(), Shows::Tab, None);
    let denoise = harness.get_all_by_role_and_label(egui::Role::Switch, text::DENOISE);
    assert_eq!(denoise.count(), 1, "only the microphone is denoised");
    click(&mut harness, egui::Role::Button, &text::mute(text::SYSTEM));
    let mute = json!({"channel": "system", "mute": true});
    assert_eq!(harness.state().edits, vec![step("audio channel", mute)]);

    harness.state_mut().recorded.clear();
    harness.run();
    assert!(harness.query_by_label(text::NO_SOUND).is_some());
}

#[test]
fn a_clip_mutes_and_follows_its_tracks() {
    let mut harness = harness(project(), Shows::Clip(1), None);
    assert!(harness.query_by_label(&text::follows(text::MIC)).is_some());
    click(&mut harness, egui::Role::RadioButton, text::SOUND_OFF);
    let mute = step("audio mute", json!({"clip": "c1"}));
    assert_eq!(harness.state().edits, vec![mute]);
}

#[test]
fn an_imported_video_shows_its_streams_once_read() {
    let mut harness = harness(project(), Shows::Clip(3), None);
    assert!(harness.query_by_label(text::NO_VIDEO_SOUND).is_none());
    assert_eq!(harness.query_all_by_label(text::VOLUME).count(), 0);

    harness.state_mut().streams = Some(Vec::new());
    harness.run();
    assert!(harness.query_by_label(text::NO_VIDEO_SOUND).is_some());

    harness.state_mut().streams = Some(vec!["audio-1".to_owned(), "audio-2".to_owned()]);
    harness.run();
    let second = text::follows(&text::video_track("audio-2"));
    assert!(harness.query_by_label(&second).is_some());
    let silent: Vec<_> = harness
        .get_all_by_role_and_label(egui::Role::Switch, text::SILENT)
        .collect();
    silent[1].click();
    harness.run();
    let mute = json!({"channel": "audio-2", "clip": "c3", "mute": true});
    assert_eq!(harness.state().edits, vec![step("audio channel", mute)]);
}

#[test]
fn an_extra_sound_edits_are_audio_set() {
    let mut p = project();
    let file = SoundFile {
        path: "assets/audio/музыка 1.mp3".to_owned(),
        duration: TimeDelta::from_secs_f64(30.0),
    };
    let add = AudioAdd {
        file,
        at: OutputTime::from_secs_f64(2.0),
        track: None,
        duration: None,
    };
    add.apply(&mut p).unwrap();
    let mut harness = harness(p, Shows::Extra(1), None);
    assert!(harness.query_by_label("музыка 1.mp3").is_some());
    let reset = dc_ui_kit::strings::inspector::reset_group(dc_ui_kit::strings::editor::BASIC);
    assert!(harness.get_by_label(&reset).accesskit_node().is_disabled());
    click(&mut harness, egui::Role::Switch, text::SILENT);
    let mute = json!({"id": "au1", "mute": true});
    assert_eq!(harness.state().edits, vec![step("audio set", mute)]);
}
