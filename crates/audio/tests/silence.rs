//! Task 4.11: silence search on synthetic pauses and on the owner's speech marked by hand.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use common::project::{project_dir, range, speech_dir, speech_project};
use common::{make, track};
use dc_audio::silence::{self, DEFAULT_DB, DEFAULT_MIN};
use dc_audio::tracks;

fn seconds(found: &[dc_core::Range<dc_core::time::Source>]) -> Vec<(f64, f64)> {
    found
        .iter()
        .map(|r| (r.start().as_secs_f64(), r.end().as_secs_f64()))
        .collect()
}

#[test]
fn finds_pauses_longer_than_the_minimum() {
    let dir = tempfile::tempdir().unwrap();
    // Tone 1 s, 0.7 s quiet, tone 1 s, 0.3 s quiet (too short), tone 0.5 s, 0.6 s quiet to the end.
    let source = "aevalsrc='0.5*sin(2*PI*440*t)*(lt(t,1)+between(t,1.7,2.7)+between(t,3,3.5))':s=48000:d=4.1";
    let path = make(dir.path(), "pauses.flac", source, &["-ac", "2"]);
    let found = silence::find(&track(&path), range(0.0, 4.1), DEFAULT_DB, DEFAULT_MIN).unwrap();
    let found = seconds(&found);
    assert_eq!(found.len(), 2, "{found:?}");
    for ((start, end), (want_start, want_end)) in found.iter().zip([(1.0, 1.7), (3.5, 4.1)]) {
        assert!(
            (start - want_start).abs() <= 0.01 && (end - want_end).abs() <= 0.01,
            "{found:?}"
        );
    }
}

#[test]
fn quiet_edges_are_kept_whatever_their_length() {
    let dir = tempfile::tempdir().unwrap();
    let source = "aevalsrc='0.5*sin(2*PI*440*t)*between(t,0.1,0.9)':s=48000:d=1";
    let path = make(dir.path(), "edges.flac", source, &[]);
    let found = silence::find(&track(&path), range(0.0, 1.0), DEFAULT_DB, DEFAULT_MIN).unwrap();
    let found = seconds(&found);
    assert_eq!(
        found.len(),
        2,
        "short quiet runs at both edges may join a neighbour: {found:?}"
    );
    assert!(found[0].0 == 0.0 && found[1].1 == 1.0, "{found:?}");
}

/// Pauses of the owner's speech marked by hand from the RMS of 50 ms blocks (2026-10-28):
/// louder than −30 dB at 4.45–4.55 and 4.65, speech 7.65–9.25, 11.15–11.7 and 11.85,
/// 14.85–15.1 and 15.45, 18.3–18.7, 18.85, 19.0–19.1 and 19.4 s; a noisy raised microphone at
/// −33…−38 dB between them. 19.1–19.4 is shorter than 0.5 s; the last pause runs to the end.
const SPEECH_PAUSES: [(f64, f64); 6] = [
    (0.0, 4.45),
    (4.7, 7.65),
    (9.25, 11.15),
    (11.9, 14.85),
    (15.5, 18.3),
    (19.45, 20.0),
];

#[test]
fn matches_hand_marked_pauses_of_speech() {
    let dir = project_dir(&speech_dir());
    let project = speech_project();
    let media = &project.clips[0].media;
    let mic = tracks(&dir, media)
        .unwrap()
        .into_iter()
        .find(|t| t.name == "mic")
        .unwrap();
    let whole = range(0.0, media.duration.as_secs_f64());
    let found = silence::find(&mic, whole, DEFAULT_DB, DEFAULT_MIN).unwrap();
    let found = seconds(&found);
    println!("{found:?}");
    assert_eq!(found.len(), SPEECH_PAUSES.len(), "{found:?}");
    for (got, want) in found.iter().zip(SPEECH_PAUSES) {
        // The end of the file is where the video ends.
        assert!(
            (got.0 - want.0).abs() <= 0.03 && (got.1 - want.1).abs() <= 0.03,
            "{got:?} vs {want:?}: {found:?}"
        );
    }
}
