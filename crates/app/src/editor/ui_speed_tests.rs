//! How long a frame of the editor UI takes with 500 regions on the timeline (task 7.34): the
//! layout (`editor_ui`) and its tessellation, as playback repaints it with the playhead moving,
//! with nothing, a cue and an annotation picked. A measurement, not a check: `cargo nextest run
//! -p dc-app --release --run-ignored only ui_speed --no-capture`. The budget is 8 ms a frame in
//! release.

use std::time::{Duration, Instant};

use dc_core::{Id, OutputTime, Rect};
use dc_fx_annotations::{Annotation, Annotations, Kind};
use dc_fx_captions::{CaptionsEffect, Cue, Word};
use dc_fx_zoom::{Mode, Zoom, ZoomEffect};
use dc_project::ProjectDir;
use eframe::egui;

use super::selection::Selected;
use super::test_project::{project, source};
use super::timeline::Row;
use super::{EditorWindow, editor_ui};

const ZOOMS: u32 = 170;
const CUES: u32 = 170;
const ANNOTATIONS: u32 = 160;
/// Frames measured in a row: 2 s of 60 fps.
const FRAMES: usize = 120;

/// The test project with 500 more regions: zooms over the first recording, cues over the second,
/// annotations on four rows.
fn crowded() -> dc_core::Project {
    let mut project = project();
    let mut zooms = project.state::<ZoomEffect>().unwrap();
    for n in 0..ZOOMS {
        let start = f64::from(n) * 0.35;
        zooms.zooms.push(Zoom {
            id: Id::from_number(100 + n),
            media: "media/screen.mp4".to_owned(),
            range: source(start, start + 0.3),
            scale: 1.5,
            focus: dc_core::Point { x: 0.5, y: 0.5 },
            mode: Mode::Fixed,
        });
    }
    zooms.ids.ensure_above(zooms.zooms.iter().map(|z| z.id));
    let mut captions = project.state::<CaptionsEffect>().unwrap();
    captions.cues.retain(|c| c.media != "media/screen-2.mp4");
    for n in 0..CUES {
        let start = f64::from(n) * 0.22;
        captions.cues.push(Cue {
            id: Id::from_number(100 + n),
            media: "media/screen-2.mp4".to_owned(),
            range: source(start, start + 0.2),
            words: vec![Word {
                text: format!("слово{n}"),
                range: source(start, start + 0.2),
            }],
        });
    }
    captions
        .cues
        .sort_by_key(|c| (c.media.clone(), c.range.start()));
    captions
        .ids
        .ensure_above(captions.cues.iter().map(|c| c.id));
    let mut annotations = project.state::<Annotations>().unwrap();
    for n in 0..ANNOTATIONS {
        let start = f64::from(n / 4) * 1.4;
        annotations.annotations.push(Annotation {
            id: Id::from_number(100 + n),
            kind: if n % 2 == 0 { Kind::Text } else { Kind::Shape },
            media: "media/screen.mp4".to_owned(),
            range: source(start, start + 1.2),
            track: 2 + n % 4,
            rect: Rect::new(0.1, 0.1, 0.3, 0.1).unwrap(),
            rotation: 0.0,
            enter: Default::default(),
            exit: Default::default(),
            body: serde_json::Map::new(),
            keys: Vec::new(),
        });
    }
    let ids = annotations.annotations.iter().map(|a| a.id);
    annotations.ids.ensure_above(ids.collect::<Vec<_>>());
    project.set_state::<ZoomEffect>(&zooms).unwrap();
    project.set_state::<CaptionsEffect>(&captions).unwrap();
    project.set_state::<Annotations>(&annotations).unwrap();
    project
}

struct State {
    _temp: tempfile::TempDir,
    editor: EditorWindow,
    /// How long `editor_ui` took, frame by frame.
    layout: Vec<Duration>,
}

fn harness() -> egui_kittest::Harness<'static, State> {
    let temp = tempfile::tempdir().unwrap();
    let dir = ProjectDir::create(temp.path().join("crowded.devcast")).unwrap();
    dir.save(&crowded()).unwrap();
    let sound = vec!["system".to_owned(), "mic".to_owned()];
    let (editor, _) = EditorWindow::open(dir, sound).unwrap();
    let state = State {
        _temp: temp,
        editor,
        layout: Vec::new(),
    };
    let keys = crate::keymap::EditorKeys::default();
    let groups = crate::settings::InspectorGroups::default();
    dc_ui_kit::testing::harness()
        .with_size(egui::vec2(1600.0, 1000.0))
        .with_max_steps(10_000)
        .build_ui_state(
            move |ui, state: &mut State| {
                let start = Instant::now();
                let layout = crate::settings::EditorLayout::default();
                editor_ui(
                    ui,
                    &mut state.editor,
                    layout,
                    (&keys, &groups),
                    Instant::now(),
                );
                state.layout.push(start.elapsed());
            },
            state,
        )
}

/// The first region of `row` as a pick.
fn first(harness: &egui_kittest::Harness<'static, State>, row: Row) -> Selected {
    let tracks = &harness.state().editor.tracks;
    let track = tracks.rows.iter().find(|t| t.row == row).unwrap();
    let region = track.regions.iter().find_map(|r| r.item).unwrap();
    Selected::Region(region)
}

/// Median and 95th percentile in ms.
fn spread(mut times: Vec<Duration>) -> (f64, f64) {
    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    (
        ms(times[times.len() / 2]),
        ms(times[times.len() * 95 / 100]),
    )
}

/// [`FRAMES`] frames with the playhead 1/60 s further each and the pointer still over the
/// timeline: prints layout and tessellation times.
fn measure(harness: &mut egui_kittest::Harness<'static, State>, picked: &str) {
    harness.run();
    let ppp = harness.ctx.pixels_per_point();
    let mut tessellation = Vec::new();
    harness.state_mut().layout.clear();
    for frame in 0..FRAMES {
        let at = OutputTime::from_secs_f64(10.0 + frame as f64 / 60.0);
        harness.state_mut().editor.view.playhead = at;
        harness.step();
        let shapes = harness.output().shapes.clone();
        let start = Instant::now();
        let meshes = harness.ctx.tessellate(shapes, ppp);
        tessellation.push(start.elapsed());
        assert!(!meshes.is_empty());
    }
    let (layout, layout95) = spread(std::mem::take(&mut harness.state_mut().layout));
    let (paint, paint95) = spread(tessellation);
    println!(
        "{picked}: layout {layout:.2} ms (p95 {layout95:.2}), tessellation {paint:.2} ms \
         (p95 {paint95:.2})"
    );
}

/// The work the UI thread does itself: the frame rate of an edit (`seek::frame_rate`, probes
/// from the cache) and the settings written by a Shift+wheel notch.
fn blocking() {
    let temp = tempfile::tempdir().unwrap();
    let dir = super::e2e_tests::copy(temp.path(), "clicks");
    let project = dir.load().unwrap();
    let start = Instant::now();
    super::playback::seek::frame_rate(&dir, &project);
    let cold = start.elapsed().as_secs_f64() * 1e3;
    let start = Instant::now();
    for _ in 0..100 {
        super::playback::seek::frame_rate(&dir, &project);
    }
    let warm = start.elapsed().as_secs_f64() * 10.0;
    println!("frame rate of an edit: first {cold:.2} ms, then {warm:.3} ms");
    let mut store = crate::settings::Store::load(&temp.path().join("settings.json")).unwrap();
    let start = Instant::now();
    for n in 0..100 {
        store
            .update(|s| s.editor.rows = 1.0 + n as f32 / 1000.0)
            .unwrap();
    }
    let write = start.elapsed().as_secs_f64() * 10.0;
    println!("settings written: {write:.2} ms");
}

#[test]
#[ignore = "a measurement"]
fn ui_speed() {
    let mut harness = harness();
    let regions: usize = harness
        .state()
        .editor
        .tracks
        .rows
        .iter()
        .map(|t| t.regions.len())
        .sum();
    println!("regions on the timeline: {regions}");
    let project = harness.state().editor.doc.project().clone();
    let start = Instant::now();
    for _ in 0..100 {
        project.state::<Annotations>().unwrap();
    }
    println!(
        "Annotations state parsed: {:.3} ms",
        start.elapsed().as_secs_f64() * 10.0
    );
    blocking();
    harness.hover_at(egui::pos2(800.0, 850.0));
    measure(&mut harness, "nothing picked");
    harness.state_mut().editor.view.panel = super::view::Panel::Captions;
    measure(&mut harness, "captions tab");
    let cue = first(&harness, Row::Captions);
    harness.state_mut().editor.view.selection.only(cue);
    measure(&mut harness, "a cue picked");
    let annotation = first(&harness, Row::Annotations(2));
    let selection = &mut harness.state_mut().editor.view.selection;
    selection.clear();
    selection.only(annotation);
    measure(&mut harness, "an annotation picked");
}
