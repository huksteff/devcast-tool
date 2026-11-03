//! Task 4.11: silence search on synthetic pauses and on the owner's speech marked by hand.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failed step must fail the test"
)]

mod common;

use common::project::{project_dir, range, speech_dir, speech_project};
use common::{ffmpeg, make, track};
use dc_audio::silence::{self, DEFAULT_MIN, FLOOR_DB};
use dc_audio::tracks;

/// v1's fixed threshold, dBFS: the synthetic tones and the hand marks below are made against it.
const V1_DB: f64 = -30.0;

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
    let found = silence::find(&track(&path), range(0.0, 4.1), V1_DB, DEFAULT_MIN).unwrap();
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
    let found = silence::find(&track(&path), range(0.0, 1.0), V1_DB, DEFAULT_MIN).unwrap();
    let found = seconds(&found);
    assert_eq!(
        found.len(),
        2,
        "short quiet runs at both edges may join a neighbour: {found:?}"
    );
    assert!(found[0].0 == 0.0 && found[1].1 == 1.0, "{found:?}");
}

/// Pauses of the owner's speech marked by hand from the RMS of 50 ms blocks (2026-10-28) at v1's −30 dB:
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
    let found = silence::find(&mic, whole, V1_DB, DEFAULT_MIN).unwrap();
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

#[test]
fn quietest_moment_is_in_the_gap_between_sounds() {
    let dir = tempfile::tempdir().unwrap();
    // Sound until 1.0 s, a 40 ms gap, sound again.
    let source = "aevalsrc='0.5*sin(2*PI*440*t)*(lt(t,1)+gte(t,1.04))':s=48000:d=2";
    let path = make(dir.path(), "gap.flac", source, &["-ac", "2"]);
    let around = dc_core::SourceTime::from_secs_f64(1.1);
    let reach = dc_core::TimeDelta::from_millis(150);
    let found = silence::quietest(&track(&path), around, reach)
        .unwrap()
        .as_secs_f64();
    // Any quiet window inside the 1.00–1.04 gap; of equals, the one nearest to 1.1.
    assert!((found - 1.035).abs() <= 0.001, "{found}");
    let calm = silence::quietest(
        &track(&path),
        dc_core::SourceTime::from_secs_f64(1.6),
        reach,
    )
    .unwrap()
    .as_secs_f64();
    assert!(
        (1.45..=1.75).contains(&calm),
        "no gap near: stays within reach, {calm}"
    );
}

#[test]
fn the_threshold_is_halfway_between_noise_and_speech() {
    // 70 blocks of noise at −58 dB, 30 of speech at −36 dB.
    let mut levels = vec![-58.0; 70];
    levels.extend([-36.0; 30]);
    assert_eq!(silence::threshold_of(levels), -47.0);
    // A system track: sound at −20 dB, then digital silence counted as the floor.
    let mut system = vec![-20.0; 50];
    system.extend([f64::NEG_INFINITY; 50]);
    assert_eq!(silence::threshold_of(system), (FLOOR_DB - 20.0) / 2.0);
    // A noisy microphone: noise −45, speech −35 dB; «speech − 15 dB» would be −50, under the noise.
    let mut noisy = vec![-45.0; 70];
    noisy.extend([-35.0; 30]);
    assert_eq!(silence::threshold_of(noisy), -40.0);
    assert_eq!(silence::threshold_of(vec![f64::NEG_INFINITY; 4]), FLOOR_DB);
    assert_eq!(silence::threshold_of(Vec::new()), FLOOR_DB);
}

/// The microphone of the speech fixture at `gain_db` as a file of its own.
fn speech_at(dir: &std::path::Path, gain_db: i32) -> dc_audio::SourceTrack {
    let project = speech_project();
    let mic = tracks(&project_dir(&speech_dir()), &project.clips[0].media)
        .unwrap()
        .into_iter()
        .find(|t| t.name == "mic")
        .unwrap();
    let path = dir.join(format!("речь {gain_db} dB.flac"));
    let volume = format!("volume={gain_db}dB");
    ffmpeg(&[
        "-i",
        mic.path.to_str().unwrap(),
        "-map",
        "0:a",
        "-af",
        &volume,
        path.to_str().unwrap(),
    ]);
    track(&path)
}

/// A quiet microphone (−15 dB) and a loud one (+6 dB) find the same pauses as the recording as
/// it is, and they are the hand-marked ones: the threshold moves with the recording's levels.
#[test]
fn pauses_do_not_depend_on_the_microphone_level() {
    let dir = tempfile::tempdir().unwrap();
    let whole = range(0.0, 20.0);
    let pauses = |gain| {
        let track = speech_at(dir.path(), gain);
        let db = silence::threshold(&track).unwrap();
        (
            db,
            seconds(&silence::find(&track, whole, db, DEFAULT_MIN).unwrap()),
        )
    };
    let (db, same) = pauses(0);
    for gain in [-15, 6] {
        let (moved, found) = pauses(gain);
        assert!(
            (moved - db - f64::from(gain)).abs() < 0.1,
            "{moved} vs {db}"
        );
        assert_eq!(found.len(), same.len(), "{gain} dB: {found:?} vs {same:?}");
        for (a, b) in found.iter().zip(&same) {
            assert!(
                (a.0 - b.0).abs() <= 0.05 && (a.1 - b.1).abs() <= 0.05,
                "{gain} dB: {found:?} vs {same:?}"
            );
        }
    }
    for want in SPEECH_PAUSES {
        let covered = same
            .iter()
            .any(|got| got.0 <= want.0 + 0.2 && got.1 >= want.1 - 0.2);
        assert!(covered, "hand-marked pause {want:?} not found: {same:?}");
    }
}
