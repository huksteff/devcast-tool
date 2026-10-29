//! `devcast transcript` and `cut w…` on an imported video with the synthesized speech of
//! `fixtures/audio/tts-ru.flac`. The words are put into the cache as the small model
//! recognized them (dc-transcribe tests/recognize.rs), so no model is needed here.

use std::path::{Path, PathBuf};

use dc_project::ProjectDir;
use dc_transcribe::{Model, Recognized, Word, words_cache_path, write_words};

use crate::registry::registry;
use crate::run::Outcome;
use crate::test_util::run_line;

/// Words of the speech, seconds, as recognized.
const WORDS: [(&str, f64, f64); 16] = [
    ("Привет!", 0.34, 1.00),
    ("Сегодня", 1.88, 2.18),
    ("я", 2.18, 2.46),
    ("покажу,", 2.46, 3.34),
    ("как", 3.34, 3.72),
    ("записать", 3.72, 4.28),
    ("видео", 4.28, 4.62),
    ("с", 4.62, 4.94),
    ("экрана", 4.94, 5.24),
    ("и", 5.24, 5.42),
    ("вырезать", 5.42, 5.96),
    ("паузы.", 5.96, 6.72),
    ("Это", 7.44, 7.76),
    ("занимает", 7.76, 8.38),
    ("пару", 8.38, 8.78),
    ("минут.", 8.78, 9.40),
];

pub(crate) struct Speech {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Speech {
    pub(crate) fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let video = tmp.path().join("речь.mp4");
        let speech = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio/tts-ru.flac");
        let ffmpeg = Path::new(env!("FFMPEG_DIR")).join("bin").join("ffmpeg.exe");
        let status = std::process::Command::new(ffmpeg)
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x240:rate=30",
                "-i",
            ])
            .arg(&speech)
            .args([
                "-t", "10", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "flac",
            ])
            .arg(&video)
            .status()
            .unwrap();
        assert!(status.success());
        let out = run_line(
            "new речь.mp4 -o Речь.dcproj",
            tmp.path(),
            &registry().unwrap(),
        );
        assert_eq!(out.code, 0, "{}", out.stderr);
        let root = tmp.path().join("Речь.dcproj");
        let dir = ProjectDir::open(&root).unwrap();
        let project = dir.load().unwrap();
        let track = dc_audio::tracks(&dir, &project.clips[0].media)
            .unwrap()
            .remove(0);
        let words = WORDS
            .iter()
            .map(|(text, start, end)| Word {
                text: (*text).to_owned(),
                start: (start * 1e6) as i64,
                end: (end * 1e6) as i64,
                probability: 0.9,
            })
            .collect();
        let recognized = Recognized {
            language: "ru".to_owned(),
            words,
        };
        write_words(
            &words_cache_path(&dir, &track, Model::Small, None),
            &track,
            &recognized,
        )
        .unwrap();
        Self { _tmp: tmp, root }
    }

    pub(crate) fn run(&self, line: &str) -> Outcome {
        run_line(line, &self.root, &registry().unwrap())
    }

    pub(crate) fn ok(&self, line: &str) -> String {
        let out = self.run(line);
        assert_eq!(out.code, 0, "{line}: {}", out.stderr);
        out.stdout.trim_end().to_owned()
    }
}

#[test]
fn cues_and_words_with_ids() {
    let speech = Speech::new();
    assert_eq!(
        speech.ok("transcript"),
        "0:00.340–0:01.000  w1  Привет!
0:01.880–0:06.720  w2–w12  Сегодня я покажу, как записать видео с экрана и вырезать паузы.
0:07.440–0:09.400  w13–w16  Это занимает пару минут."
    );
    assert_eq!(
        speech.ok("transcript --words --from 7 --to 8"),
        "w13  0:07.440–0:07.760  Это
w14  0:07.760–0:08.380  занимает"
    );
    let json: serde_json::Value =
        serde_json::from_str(&speech.ok("transcript --from 8 --json")).unwrap();
    assert_eq!(json["language"], serde_json::json!(["ru"]));
    assert_eq!(json["items"][0]["words"], serde_json::json!(["w13", "w16"]));
}

#[test]
fn cutting_words_keeps_their_ids_and_snaps_to_quiet() {
    let speech = Speech::new();
    let cut = speech.ok("cut w2-w12");
    assert!(cut.contains("длительность"), "{cut}");
    let after = speech.ok("transcript");
    let lines: Vec<&str> = after.lines().collect();
    assert_eq!(lines.len(), 3, "{after}");
    assert!(
        lines[1].ends_with("w13–w16  Это занимает пару минут."),
        "{after}"
    );
    assert_eq!(lines[2], "вырезано и не показано слов: 11");
    // «Это» moved left by the cut of 1.88–6.72 give or take the snap (≤ 0.15 s each side).
    let start: f64 = lines[1][2..8].parse().unwrap();
    assert!((start - (7.44 - 4.84)).abs() <= 0.3, "{after}");
}

#[test]
fn word_ids_must_exist_and_be_recognized_first() {
    let speech = Speech::new();
    let out = speech.run("cut w20-w21");
    assert_eq!(out.code, 1);
    assert!(
        out.stderr.contains("w20 не найдено (есть w1..w16)"),
        "{}",
        out.stderr
    );
    let out = speech.run("cut w5-w3");
    assert_eq!(out.code, 2, "{}", out.stderr);
    assert!(out.stderr.contains("по порядку"), "{}", out.stderr);
    let out = speech.run("cut w1-w2-w3");
    assert_eq!(out.code, 2, "{}", out.stderr);
    assert!(out.stderr.contains("диапазон w12-w20"), "{}", out.stderr);
    let out = speech.run("cut w99999999999");
    assert_eq!(out.code, 2, "{}", out.stderr);
    assert!(out.stderr.contains("такого номера"), "{}", out.stderr);
    std::fs::remove_dir_all(speech.root.join("cache")).unwrap();
    let out = speech.run("cut w1");
    assert!(
        out.stderr.contains("сначала devcast transcript"),
        "{}",
        out.stderr
    );
}
