//! Waves of the sound rows (7.11): which source frames each column shows through a cut and a
//! 4× range, the levels and the pale muted rows; snapshots with the peaks of a generated tone.

use dc_core::{ClipId, OutputTime, TimeDelta, TimeMap};

use super::clips_tests::{H, lanes};
use super::scale::TimeScale;
use super::tracks::{Row, Tracks};
use super::wave::{self, COLUMN, Column, Source};
use crate::editor::test_project::project;
use crate::editor::waves_tests::tone_peaks;

/// 48 kHz frames of source second `secs`.
fn frame(secs: f64) -> i64 {
    (secs * 48_000.0).round() as i64
}

fn scale(px_per_sec: f32) -> TimeScale {
    TimeScale {
        left: 0.0,
        start: OutputTime::ZERO,
        px_per_sec,
    }
}

/// The microphone of the first clip: source 0–60 s, cut 20–25 s, 4× at 30–38 s.
fn mic() -> wave::Wave {
    let p = project();
    let map = TimeMap::build(&p);
    wave::recording(&map, ClipId::from_number(1), &p.clips[0].media, "mic", 1.0)
}

/// The column whose left edge is at output second `secs` (a column is 0.2 s at 10 px/s).
fn column_at(columns: &[Column], secs: f64) -> Column {
    let x = (secs * 10.0) as f32 + COLUMN / 2.0;
    *columns.iter().find(|c| (c.x - x).abs() < 1e-3).unwrap()
}

#[test]
fn columns_follow_the_time_map_through_a_cut_and_a_speed() {
    let wave = mic();
    assert_eq!(wave.spans.len(), 4, "before, after the cut, 4×, after it");
    let columns = wave::columns(&wave, &scale(10.0), 0.0, 600.0);
    assert_eq!(column_at(&columns, 0.0).frames, (0, frame(0.2)));
    // Output 20 s is source 25 s: the cut 20–25 s is not drawn.
    assert_eq!(column_at(&columns, 20.0).frames, (frame(25.0), frame(25.2)));
    // Output 25 s is source 30 s at 4×: a column shows four times as much sound.
    assert_eq!(column_at(&columns, 25.0).frames, (frame(30.0), frame(30.8)));
    assert_eq!(
        column_at(&columns, 27.0).frames,
        (frame(38.0), frame(38.2)),
        "1× again after the range"
    );
    let end = columns.last().unwrap();
    assert_eq!(end.frames.1, frame(60.0), "the clip ends at output 49 s");
    assert!(columns.iter().all(|c| c.x < 49.0 * 10.0 + COLUMN));
}

#[test]
fn a_column_over_a_seam_shows_only_its_own_span() {
    // At 2.5 px/s the column 62–64 px is output 24.8–25.6 s; its middle is in the 4× span.
    let columns = wave::columns(&mic(), &scale(2.5), 62.0, 64.0);
    assert_eq!(columns.len(), 1);
    assert_eq!(columns[0].frames, (frame(30.0), frame(32.4)));
}

#[test]
fn columns_stay_on_their_times_while_the_view_scrolls() {
    let wave = mic();
    let at = |start: f64, from: f32| {
        let scale = TimeScale {
            left: 0.0,
            start: OutputTime::from_secs_f64(start),
            px_per_sec: 10.0,
        };
        wave::columns(&wave, &scale, from, from + 40.0)
    };
    let (still, scrolled) = (at(10.0, 0.0), at(10.3, 0.0));
    assert_eq!(still[0].frames, (frame(10.0), frame(10.2)));
    assert!(scrolled[0].x.abs() < 1e-3, "{:?}", scrolled[0]);
    assert_eq!(
        scrolled[0].frames,
        (frame(10.2), frame(10.4)),
        "the grid holds"
    );
    assert_eq!(scrolled[1].frames, (frame(10.4), frame(10.6)));
}

#[test]
fn extra_sound_plays_its_file_from_the_offset_at_its_level() {
    let p = project();
    let sound = dc_fx_audio_tracks::sound(&p).unwrap();
    let state = p.state::<dc_fx_audio_tracks::Audio>().unwrap();
    let mut clip = state.clips[0].clone();
    clip.offset = TimeDelta::from_secs_f64(10.0);
    let wave = wave::extra(&clip);
    assert_eq!(wave.source, Source::File(clip.file.path.clone()));
    assert!((wave.level - 0.3).abs() < 1e-6, "volume 30 %");
    let columns = wave::columns(&wave, &scale(10.0), 0.0, 4.0);
    assert_eq!(columns[0].frames, (frame(10.0), frame(10.2)));
    assert_eq!(sound.extra.len(), 1);
}

#[test]
fn sound_rows_carry_their_waves_and_mutes() {
    let sound = ["system".to_owned(), "mic".to_owned()];
    let tracks = Tracks::of(&project(), &sound, Default::default());
    let row = |row| tracks.rows.iter().find(|t| t.row == row).unwrap();
    let mic = row(Row::Mic);
    assert_eq!(
        mic.regions.len(),
        2,
        "the recordings, not the imported video"
    );
    assert!(mic.regions.iter().all(|r| !r.faded));
    let system = row(Row::System);
    assert!(system.regions.iter().all(|r| r.faded), "muted: pale");
    let waves: Vec<_> = tracks.sources().collect();
    assert_eq!(
        waves.len(),
        5,
        "mic and system of two recordings, the music"
    );
}

/// The editor with every row shown and the peaks of a tone for every source.
fn with_peaks() -> H {
    let layout = crate::settings::EditorLayout {
        timeline_height: 560.0,
        ..Default::default()
    };
    let size = eframe::egui::vec2(1280.0, 1000.0);
    let mut harness = crate::editor::tests::harness_with(size, layout);
    harness.run();
    assert!(harness.state().editor.waves.peaks(&mic().source).is_none());
    let peaks = tone_peaks(110.0);
    let editor = &mut harness.state_mut().editor;
    let sources: Vec<Source> = editor.tracks.sources().cloned().collect();
    for source in sources {
        editor.waves.arrive(source, Ok(peaks.clone()));
    }
    harness.run();
    harness
}

#[test]
fn snapshots_of_the_waves() {
    let mut harness = with_peaks();
    harness.snapshot("editor_waves");
    let lanes = lanes(&harness);
    let view = &mut harness.state_mut().editor.view.timeline;
    let anchor = view.scale(&lanes).x(OutputTime::from_secs_f64(25.0));
    view.zoom_by(&lanes, 12.0, anchor);
    harness.run();
    harness.snapshot("editor_waves_zoomed");
}
