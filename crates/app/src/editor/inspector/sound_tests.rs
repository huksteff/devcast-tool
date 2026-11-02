//! The sound panels (7.19): the «Звук» tab edits `audio channel <track>`, the sound of a clip —
//! picked on its stretch of «Микрофон» — `audio mute` and `audio channel --clip`, an extra sound
//! clip `audio set`; each edit is one undo step and the same project as `devcast`.

use dc_core::ClipId;
use dc_fx_audio_tracks::{Audio, AudioId};
use dc_ui_kit::strings::{annotations, inspector as names, sound as text};
use eframe::egui;
use egui_kittest::kittest::{NodeT as _, Queryable};

use super::tests::{H, cli, pick, project_json, undo};
use super::zoom_tests::{click, tall};
use crate::editor::editing_tests::json;
use crate::editor::selection::{Item, Selected};
use crate::editor::timeline::Row;
use crate::editor::view::Panel;

fn sound_tab() -> H {
    let mut harness = tall();
    harness.state_mut().editor.view.panel = Panel::Sound;
    harness.run();
    harness
}

/// Types `value` into the `nth` number field named `label`.
fn type_nth(harness: &mut H, label: &str, nth: usize, value: f64) {
    let fields: Vec<_> = harness
        .get_all_by_role_and_label(egui::Role::SpinButton, label)
        .collect();
    fields[nth].set_value_accesskit(value);
    harness.run();
}

/// Clicks the `nth` switch named `label`.
fn switch_nth(harness: &mut H, label: &str, nth: usize) {
    let switches: Vec<_> = harness
        .get_all_by_role_and_label(egui::Role::Switch, label)
        .collect();
    switches[nth].click();
    harness.run();
}

#[test]
fn sound_tab_edits_are_audio_channel() {
    let mut harness = sound_tab();
    let denoise = harness.get_all_by_role_and_label(egui::Role::Switch, text::DENOISE);
    assert_eq!(denoise.count(), 1, "only the microphone is denoised");
    type_nth(&mut harness, text::VOLUME, 0, 80.0);
    switch_nth(&mut harness, text::NORMALIZE, 0);
    switch_nth(&mut harness, text::DENOISE, 0);
    click(
        &mut harness,
        egui::Role::Button,
        &text::unmute(text::SYSTEM),
    );
    type_nth(&mut harness, text::GAIN, 1, 2.5);
    let lines = [
        "audio channel mic --volume 80",
        "audio channel mic --normalize true",
        "audio channel mic --denoise true",
        "audio channel system --mute false",
        "audio channel system --gain 2.5",
    ];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    harness.snapshot("inspector_sound");
    undo(&mut harness);
    assert_eq!(
        project_json(&harness),
        json(&cli(&lines[..4])),
        "each edit is one step"
    );
}

#[test]
fn sound_tab_reset_brings_a_track_back_in_one_step() {
    let mut harness = sound_tab();
    let mic = names::reset_group(text::MIC);
    assert!(harness.get_by_label(&mic).accesskit_node().is_disabled());
    type_nth(&mut harness, text::VOLUME, 0, 40.0);
    switch_nth(&mut harness, text::DENOISE, 0);
    let edited = [
        "audio channel mic --volume 40",
        "audio channel mic --denoise true",
    ];
    harness.get_by_label(&mic).click();
    harness.run();
    let mut lines = edited.to_vec();
    lines.push(
        "audio channel mic --volume 100 --gain 1 --mute false --normalize false --denoise false",
    );
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&edited)), "one step back");
}

/// The editor with the timeline tall enough for the sound rows.
fn tall_timeline() -> H {
    let layout = crate::settings::EditorLayout {
        timeline_height: 560.0,
        ..Default::default()
    };
    let mut harness = crate::editor::tests::harness_with(egui::vec2(1280.0, 1000.0), layout);
    harness.run();
    harness
}

fn mic_faded(harness: &H, clip: ClipId) -> bool {
    let rows = &harness.state().editor.tracks.rows;
    let mic = rows.iter().find(|t| t.row == Row::Mic).unwrap();
    mic.regions
        .iter()
        .find(|r| r.clip == Some(clip))
        .unwrap()
        .faded
}

#[test]
fn a_click_on_the_mic_row_picks_the_clip() {
    let mut harness = tall_timeline();
    let first = ClipId::from_number(1);
    let stretch = names::on_row(text::MIC, &dc_ui_kit::strings::timeline::clip(1));
    click(&mut harness, egui::Role::Button, &stretch);
    assert_eq!(
        harness.state().editor.view.selection.single(),
        Some(Selected::Clip(first))
    );
    harness.snapshot("editor_clip_sound_rows");
}

#[test]
fn a_picked_clip_edits_its_own_levels() {
    let first = ClipId::from_number(1);
    let mut harness = tall();
    pick(&mut harness, Some(Selected::Clip(first)));
    assert!(harness.query_by_label(&text::follows(text::MIC)).is_some());
    type_nth(&mut harness, text::VOLUME, 0, 40.0);
    switch_nth(&mut harness, text::SILENT, 1);
    assert!(
        harness
            .query_by_label(&text::own(text::MIC, &[text::OWN_VOLUME]))
            .is_some(),
        "the caption names the levels of its own"
    );
    harness.snapshot("inspector_clip_sound");
    click(&mut harness, egui::Role::RadioButton, text::SOUND_OFF);
    let lines = [
        "audio channel mic --clip c1 --volume 40",
        "audio channel system --clip c1 --mute false",
        "audio mute c1",
    ];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    assert!(
        mic_faded(&harness, first),
        "the wave of a silent clip is pale"
    );
    undo(&mut harness);
    assert_eq!(project_json(&harness), json(&cli(&lines[..2])));
    click(&mut harness, egui::Role::RadioButton, text::SOUND_OFF);
    harness
        .get_by_label(&names::reset_group(text::CLIP_SOUND))
        .click();
    harness.run();
    let mut reset = lines.to_vec();
    reset.extend(["audio reset c1", "audio mute c1 --off"]);
    assert_eq!(project_json(&harness), json(&cli(&reset)));
    undo(&mut harness);
    assert_eq!(
        project_json(&harness),
        json(&cli(&lines)),
        "reset is one step"
    );
}

fn extra(harness: &mut H) {
    let id = AudioId::from_number(1);
    pick(harness, Some(Selected::Region(Item::Sound(id))));
}

#[test]
fn an_extra_sound_edits_are_audio_set() {
    let mut harness = tall();
    extra(&mut harness);
    let file = harness.query_by_role_and_label(egui::Role::Label, "музыка_фон.mp3");
    assert!(file.is_some(), "the panel names the file");
    type_nth(&mut harness, text::VOLUME, 0, 50.0);
    switch_nth(&mut harness, text::NORMALIZE, 0);
    switch_nth(&mut harness, text::SILENT, 0);
    type_nth(&mut harness, text::LENGTH, 0, 20.0);
    type_nth(&mut harness, text::START, 0, 2.0);
    type_nth(&mut harness, text::OFFSET, 0, 10.0);
    let lines = [
        "audio set au1 --volume 50",
        "audio set au1 --normalize true",
        "audio set au1 --mute true",
        "audio set au1 --duration 20",
        "audio set au1 --at 2",
        "audio set au1 --offset 10",
    ];
    assert_eq!(project_json(&harness), json(&cli(&lines)));
    harness.snapshot("inspector_sound_extra");
    harness
        .get_by_label(&names::reset_group(dc_ui_kit::strings::editor::BASIC))
        .click();
    harness.run();
    let mut reset = lines.to_vec();
    reset.push("audio set au1 --volume 100 --gain 1 --normalize false --mute false");
    assert_eq!(project_json(&harness), json(&cli(&reset)));
    undo(&mut harness);
    assert_eq!(
        project_json(&harness),
        json(&cli(&lines)),
        "reset is one step"
    );
}

#[test]
fn duplicate_puts_a_copy_of_the_sound_on_a_free_row_and_picks_it() {
    let mut harness = tall();
    extra(&mut harness);
    click(&mut harness, egui::Role::Button, annotations::DUPLICATE);
    let project = harness.state().editor.doc.project();
    let state = project.state::<Audio>().unwrap();
    let (first, copy) = (&state.clips[0], &state.clips[1]);
    assert_eq!((copy.track, copy.range), (1, first.range));
    assert_eq!((copy.volume, &copy.file), (first.volume, &first.file));
    let picked = Selected::Region(Item::Sound(copy.id));
    assert_eq!(harness.state().editor.view.selection.single(), Some(picked));
    undo(&mut harness);
    let now = harness.state().editor.doc.project();
    let expected = crate::editor::editing_tests::undone(&cli(&[]), now);
    assert_eq!(json(now), expected, "one step");
}
