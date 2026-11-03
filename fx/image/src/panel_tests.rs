//! The image section of the annotation panel on its own (`dc_fx_annotations::panel::ui` with
//! `ImageBody`): the file and its size, «Заменить…» asks the editor for a picture file
//! (`Request::Picture`) for the key at the playhead or everywhere. Copying the file and
//! `image set` are the editor's (`inspector/annot_kinds_tests.rs` in dc-app).

use std::collections::BTreeMap;

use dc_core::{OutputTime, PixelSize, Project, Range, Step};
use dc_fx_annotations::AnnotId;
use dc_fx_annotations::panel::{self, Region};
use dc_ui_kit::egui;
use dc_ui_kit::panel::{Cx, Groups, Request};
use dc_ui_kit::strings::annotations as text;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use crate::ImageBody;

struct State {
    project: Project,
    defaults: Project,
    groups: BTreeMap<String, bool>,
    playhead: OutputTime,
    /// The timeline's keys of a1, output times.
    keys: Vec<OutputTime>,
    edits: Vec<Vec<Step>>,
    requests: Vec<Request>,
}

fn at(secs: f64) -> OutputTime {
    OutputTime::from_secs_f64(secs)
}

/// The panel of a1 of `project` (shown at 1–2 s of the output) with the playhead at
/// `playhead` s and the timeline's keys of a1 at `keys` s.
fn harness(project: Project, playhead: f64, keys: &[f64]) -> Harness<'static, State> {
    let state = State {
        project,
        defaults: Project::default(),
        groups: BTreeMap::new(),
        playhead: at(playhead),
        keys: keys.iter().map(|k| at(*k)).collect(),
        edits: Vec::new(),
        requests: Vec::new(),
    };
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(340.0, 1400.0))
        .build_ui_state(draw, state);
    harness.run();
    harness
}

fn draw(ui: &mut egui::Ui, state: &mut State) {
    let fonts = ["Inter".to_owned(), "Montserrat".to_owned()];
    let mut cx = Cx {
        project: &state.project,
        defaults: &state.defaults,
        playhead: state.playhead,
        auto_keys: false,
        fonts: &fonts,
        groups: Groups::new(&mut state.groups),
        edits: Vec::new(),
        adds: Vec::new(),
        requests: Vec::new(),
    };
    let region = Region {
        range: Range::new(at(1.0), at(2.0)).unwrap(),
        keys: &state.keys,
        canvas: Some(PixelSize {
            width: 1920,
            height: 1080,
        }),
    };
    panel::ui::<ImageBody>(ui, &mut cx, AnnotId::from_number(1), &region);
    let (edits, requests) = (cx.edits, cx.requests);
    state.edits.extend(edits);
    state.requests.extend(requests);
}

fn click(harness: &mut Harness<'_, State>, role: egui::Role, label: &str) {
    harness.get_by_role_and_label(role, label).click();
    harness.run();
}

#[test]
fn replace_asks_for_a_picture_everywhere_or_for_the_key() {
    let mut harness = harness(crate::tests::project(), 1.5, &[]);
    assert!(harness.query_by_label(text::NO_FILE).is_some());
    click(&mut harness, egui::Role::Button, text::REPLACE);
    let everywhere = Request::Picture {
        id: "a1".to_owned(),
        at: None,
    };
    assert_eq!(harness.state().requests, vec![everywhere]);
    let mut keyed = self::harness(crate::tests::project(), 1.25, &[1.25]);
    click(&mut keyed, egui::Role::Button, text::REPLACE);
    let at_key = Request::Picture {
        id: "a1".to_owned(),
        at: Some(at(1.25)),
    };
    assert_eq!(keyed.state().requests, vec![at_key]);
    assert!(keyed.state().edits.is_empty(), "no registry command");
}

#[test]
fn the_file_is_named_with_its_size_and_basic_has_no_reset() {
    let mut project = crate::tests::project();
    let set = crate::ImageSet {
        id: AnnotId::from_number(1),
        picture: crate::tests::picture("sticker.png", 160, 100),
        at: None,
    };
    dc_core::Command::apply(&set, &mut project).unwrap();
    let harness = harness(project, 1.5, &[]);
    let name = text::picture("sticker.png", 160, 100);
    assert!(harness.query_by_label(&name).is_some(), "«{name}»");
    let reset = dc_ui_kit::strings::inspector::reset_group(dc_ui_kit::strings::editor::BASIC);
    assert!(harness.query_by_label(&reset).is_none());
}
