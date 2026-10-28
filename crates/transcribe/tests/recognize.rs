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
