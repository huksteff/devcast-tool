//! Task 4.13 on the real model `.deps/models/ggml-small.bin` (not in CI: 488 MB):
//! `cargo nextest run -p dc-transcribe --run-ignored only`.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failed step must fail the test"
)]

use std::path::Path;

use dc_audio::{RATE, SourceTrack};
use dc_transcribe::{Model, cues, recognize, speech_samples};

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// The synthesized speech of `fixtures/audio/tts-ru.flac`.
const SAID: &str = "Привет. Сегодня я покажу, как записать видео с экрана и вырезать паузы. Это занимает пару минут.";

fn words_of(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

#[test]
#[ignore = "needs .deps/models/ggml-small.bin"]
fn recognizes_russian_speech_with_word_times() {
    let track = SourceTrack {
        name: "speech".to_owned(),
        path: root().join("fixtures/audio/tts-ru.flac"),
        stream: 0,
        origin: 0,
    };
    let samples = speech_samples(&[track], 0, 10 * i64::from(RATE)).unwrap();
    let model = root().join(".deps/models/ggml-small.bin");
    let started = std::time::Instant::now();
    let recognized = recognize(&model, Model::Small, &samples, None, |_| {}).unwrap();
    let text: Vec<&str> = recognized.words.iter().map(|w| w.text.as_str()).collect();
    println!(
        "{:.1} s: [{}] {}",
        started.elapsed().as_secs_f64(),
        recognized.language,
        text.join(" ")
    );
    for word in &recognized.words {
        println!(
            "  {:>6.2}–{:<6.2} {:.2} {}",
            word.start as f64 / 1e6,
            word.end as f64 / 1e6,
            word.probability,
            word.text
        );
    }
    assert_eq!(recognized.language, "ru");
    assert_eq!(words_of(&text.join(" ")), words_of(SAID));
    for pair in recognized.words.windows(2) {
        assert!(pair[0].start <= pair[1].start && pair[0].end <= pair[1].start);
    }
    let cues = cues(&recognized.words, &[]);
    let lines: Vec<String> = cues.iter().map(|c| c.text(&recognized.words)).collect();
    println!("{lines:#?}");
    assert_eq!(lines.len(), 3, "three sentences");
    // Onsets of the three sentences by silencedetect at −40 dB: 0.27, 1.72, 7.49 s.
    for (cue, onset) in cues.iter().zip([0.27, 1.72, 7.49]) {
        let first = &recognized.words[cue.words[0]];
        let error = first.start as f64 / 1e6 - onset;
        assert!(
            error.abs() < 0.2,
            "«{}» starts {error:+.2} s off",
            first.text
        );
    }
}

/// The owner speaking into the microphone (`fixtures/recordings/speech_owner.devcast`, 45 s):
/// recognized as Russian; a cue ending in a pause ends where the quiet starts, though Whisper
/// stretches its last word over the pause. Pauses by silencedetect at
/// −45 dB, at least 0.8 s, on its `mic.mp4`: seconds from the file start.
const OWNER_PAUSES: [(f64, f64); 3] = [(12.78, 13.61), (16.23, 17.70), (22.29, 23.33)];

#[test]
#[ignore = "needs .deps/models/ggml-small.bin"]
fn the_owners_cues_end_where_the_pauses_start() {
    let track = SourceTrack {
        name: "mic".to_owned(),
        path: root().join("fixtures/recordings/speech_owner.devcast/media/recording-1/mic.mp4"),
        stream: 0,
        origin: 0,
    };
    let samples = speech_samples(&[track], 0, 45 * i64::from(RATE)).unwrap();
    let model = root().join(".deps/models/ggml-small.bin");
    let recognized = recognize(&model, Model::Small, &samples, None, |_| {}).unwrap();
    for word in &recognized.words {
        println!(
            "  {:>6.2}–{:<6.2} {}",
            word.start as f64 / 1e6,
            word.end as f64 / 1e6,
            word.text
        );
    }
    assert_eq!(recognized.language, "ru");
    assert!(
        recognized.words.len() >= 30,
        "{} words",
        recognized.words.len()
    );
    // Whisper's word times are rounded to its 20 ms frames and drift a little at the edges.
    const SLACK: f64 = 0.25;
    let silences: Vec<(i64, i64)> = OWNER_PAUSES
        .iter()
        .map(|(from, to)| ((from * 1e6) as i64, (to * 1e6) as i64))
        .collect();
    let found = cues(&recognized.words, &silences);
    let pad = dc_transcribe::EDGE_PAD as f64 / 1e6;
    for (from, to) in OWNER_PAUSES {
        // A cue ending in a pause ends where it starts, plus its padding; a pause inside a
        // sentence (shorter than a splitting silence) keeps its cue on screen.
        let late: Vec<String> = found
            .iter()
            .filter(|c| {
                let end = c.end as f64 / 1e6;
                end > from + pad + SLACK && end < to
            })
            .map(|c| c.text(&recognized.words))
            .collect();
        assert!(
            late.is_empty(),
            "cues ending late in the pause {from}–{to}: {late:?}"
        );
        let next = recognized
            .words
            .iter()
            .map(|w| w.start as f64 / 1e6)
            .find(|start| *start >= from)
            .unwrap();
        assert!(
            (next - to).abs() < 0.4,
            "speech after the pause {from}–{to} starts at {next:.2}"
        );
    }
}
