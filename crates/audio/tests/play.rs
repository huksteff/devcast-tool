//! Task 7.5: the mix played through a line into a fake output sink (nothing reaches the
//! speakers): the sink gets exactly the mix from the start frame, at the volume, and the line's
//! clock counts what the sink played.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::clip;
use common::project::{add_clip, mixer, render_all};
use dc_audio::{CHANNELS, Levels, Line, Mixer, Off, RATE, feed};
use dc_core::Project;

const TONE: &str = "aevalsrc=0.5*sin(2*PI*440*t)|0.25*sin(2*PI*660*t):s=48000:d=2";

/// Frames of one device period of the fake sink (10 ms).
const PERIOD: usize = 480;

/// A tone project in a temporary folder and its mixer.
fn tone() -> (tempfile::TempDir, Project, Mixer) {
    let dir = tempfile::tempdir().unwrap();
    clip(dir.path(), "tone.mp4", TONE, &[]);
    let mut project = Project::default();
    add_clip(&mut project, "tone.mp4", 2.0);
    let mixer = mixer(dir.path(), &project, &Levels::default());
    (dir, project, mixer)
}

/// Plays `line` fed by `mixer` from `from` into a fake sink until the end; returns what the
/// sink played (the silence of an empty queue left out, by the line's clock).
fn play(mut mixer: Mixer, line: &Arc<Line>, from: i64) -> Vec<f32> {
    let feeder = Arc::clone(line);
    let thread = std::thread::spawn(move || feed(&mut mixer, &feeder, from).unwrap());
    let start = Instant::now();
    let long_after = start + Duration::from_secs(3600);
    let mut played = Vec::new();
    let mut out = vec![0.0; PERIOD * CHANNELS];
    while !line.finished(long_after) {
        let before = line.heard(long_after);
        line.pull(&mut out, start);
        let taken = (line.heard(long_after) - before) as usize * CHANNELS;
        played.extend_from_slice(&out[..taken]);
        assert!(
            out[taken..].iter().all(|s| *s == 0.0),
            "silence after the sound"
        );
        std::thread::yield_now();
    }
    line.close();
    thread.join().unwrap();
    played
}

#[test]
fn the_sink_gets_the_mix_from_the_start_frame() {
    let (dir, project, mixer) = tone();
    let expected = render_all(
        &mut common::project::mixer(dir.path(), &project, &Levels::default()),
        1024,
    );
    let from = RATE as i64 / 2;
    let total = mixer.frames();
    let line = Arc::new(Line::new(1.0));
    let played = play(mixer, &line, from);
    assert_eq!(played.len(), (total - from) as usize * CHANNELS);
    let wanted = &expected[from as usize * CHANNELS..total as usize * CHANNELS];
    let worst = played
        .iter()
        .zip(wanted)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(worst < 1e-6, "differs from the mix by {worst}");
    assert!(common::peak(&played) > 0.4, "the tone sounds");
}

#[test]
fn the_volume_scales_the_sound() {
    let (dir, project, mixer) = tone();
    let full = play(mixer, &Arc::new(Line::new(1.0)), 0);
    let quiet = play(
        common::project::mixer(dir.path(), &project, &Levels::default()),
        &Arc::new(Line::new(0.25)),
        0,
    );
    assert_eq!(full.len(), quiet.len());
    for (a, b) in full.iter().zip(&quiet) {
        assert!((a * 0.25 - b).abs() < 1e-6);
    }
}

#[test]
fn the_clock_counts_played_frames_only() {
    let line = Line::new(1.0);
    let t0 = Instant::now();
    assert_eq!(line.heard(t0), 0, "nothing played yet");
    let mut out = vec![1.0; PERIOD * CHANNELS];
    line.pull(&mut out, t0);
    assert!(
        out.iter().all(|s| *s == 0.0),
        "an empty queue plays silence"
    );
    assert_eq!(
        line.heard(t0 + Duration::from_secs(1)),
        0,
        "silence is not counted"
    );
}

#[test]
fn closing_the_line_stops_the_feed() {
    let (_dir, _project, mut mixer) = tone();
    let line = Arc::new(Line::new(1.0));
    let feeder = Arc::clone(&line);
    let thread = std::thread::spawn(move || feed(&mut mixer, &feeder, 0));
    line.close();
    thread.join().unwrap().unwrap();
    assert!(!line.finished(Instant::now()), "stopped before the end");
}

#[test]
fn a_line_turned_off_says_why_and_closes() {
    let line = Line::new(1.0);
    line.turn_off(Off::Failed("нет устройства".to_owned()));
    assert_eq!(line.off(), Some(Off::Failed("нет устройства".to_owned())));
    line.wait_closed();
}

#[test]
fn a_project_without_sound_is_silent() {
    let dir = tempfile::tempdir().unwrap();
    let project = Project::default();
    assert!(mixer(dir.path(), &project, &Levels::default()).is_silent());
    let (_dir, _project, tone) = tone();
    assert!(!tone.is_silent());
}
