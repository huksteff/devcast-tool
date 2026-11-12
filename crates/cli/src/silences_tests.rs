//! `devcast silences` on a copy of the owner's speech recording (pauses marked by hand in
//! `dc-audio` tests/silence.rs).

use std::path::Path;

use crate::run::Outcome;
use crate::test_util::run_line;

struct Speech {
    _tmp: tempfile::TempDir,
    root: std::path::PathBuf,
}

impl Speech {
    fn new() -> Self {
        Self::of("speech.devcast")
    }

    /// A copy of the recording `name` of the fixtures.
    fn of(name: &str) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Речь с паузами.devcast");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/recordings")
            .join(name);
        crate::test_util::copy_dir(&fixture, &root);
        Self { _tmp: tmp, root }
    }

    fn run(&self, line: &str) -> Outcome {
        run_line(line, &self.root, &dc_ops::Registry::devcast().unwrap())
    }

    fn ok(&self, line: &str) -> String {
        let out = self.run(line);
        assert_eq!(out.code, 0, "{line}: {}", out.stderr);
        out.stdout.trim_end().to_owned()
    }
}

#[test]
fn lists_pauses_of_the_microphone() {
    let speech = Speech::new();
    assert_eq!(
        speech.ok("silences"),
        "0:00.000–0:04.450  4.45 с
0:04.700–0:07.650  2.95 с
0:09.250–0:11.150  1.90 с
0:11.900–0:14.850  2.95 с
0:15.500–0:18.300  2.80 с
0:19.450–0:20.016  0.57 с
тишина: 6, всего 15.62 с"
    );
    assert_eq!(
        speech.ok("silences --min 2.9 --from 5 --to 14"),
        "0:04.700–0:07.650  2.95 с
0:11.900–0:14.850  2.95 с
тишина: 2, всего 5.90 с"
    );
    let json: serde_json::Value =
        serde_json::from_str(&speech.ok("silences --min 2.9 --to 8 --json")).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"total": 2, "seconds": 7.4, "items": [
            {"from": 0.0, "to": 4.45, "duration": 4.45},
            {"from": 4.7, "to": 7.65, "duration": 2.95},
        ]})
    );
}

#[test]
fn pauses_join_across_a_cut() {
    let speech = Speech::new();
    speech.ok("cut 4-12");
    // Quiet 0–4 and 12–14.85 of the recording meet at the cut: one pause of 6.85 s.
    let first = speech.ok("silences");
    assert!(first.starts_with("0:00.000–0:06.850  6.85 с\n"), "{first}");
}

#[test]
fn wrong_options_explain_what_to_write() {
    let speech = Speech::new();
    for (line, hint) in [
        ("silences --min 0", "--min"),
        ("silences --db 5", "--db"),
        ("silences --track video", "есть: system, mic"),
        ("silences --from soon", "--from"),
    ] {
        let out = speech.run(line);
        assert_eq!(out.code, 2, "{line}: {}", out.stderr);
        assert!(out.stderr.contains(hint), "{line}: {}", out.stderr);
    }
    let system = speech.ok("silences --track system --min 0.5");
    // The system track is digital silence but for a sound at 10.5–11.3 s (fading to −73 dB).
    assert!(system.contains("0:00.000–0:10.500"), "{system}");
    assert!(system.contains("0:11.300–0:20.016"), "{system}");
}

/// `cut --silences`: the pauses go by one command, `--pad` of each stays at both of its edges.
#[test]
fn pauses_are_cut_by_one_command() {
    let speech = Speech::new();
    let info = speech.ok("info");
    assert!(
        info.contains(
            "
голос: есть
"
        ),
        "{info}"
    );
    let dry = speech.ok("cut --silences --dry-run");
    assert!(dry.contains("вырез 0:00.150–0:04.300"), "{dry}");
    speech.ok("cut --silences --pad 0.2 --min 1");
    // Five pauses of 1 s and more, 15.05 s together, less 0.4 s left of each: 20.016 − 13.05.
    let json: serde_json::Value = serde_json::from_str(&speech.ok("info --json")).unwrap();
    let left = json["duration"].as_f64().unwrap();
    assert!((left - 6.966).abs() < 0.01, "{left}");
    assert_eq!(json["voice"], true);

    for (line, code, hint) in [
        ("cut 1-2 --silences", 2, "либо диапазоны"),
        ("cut --silences --pad many", 2, "--pad"),
        ("cut --silences --min 30", 1, "вырезать нечего"),
    ] {
        let out = speech.run(line);
        assert_eq!(out.code, code, "{line}: {}", out.stderr);
        assert!(out.stderr.contains(hint), "{line}: {}", out.stderr);
    }
}

/// A recording without a microphone has no pauses of speech: `info` says so, `cut --silences`
/// cuts nothing, `silences` warns what it lists.
#[test]
fn a_recording_without_a_voice_is_not_cut() {
    let silent = Speech::of("system_sound.devcast");
    let info = silent.ok("info");
    assert!(
        info.contains("голос: нет (media/recording-1/video.mp4: записано без микрофона)"),
        "{info}"
    );
    let json: serde_json::Value = serde_json::from_str(&silent.ok("info --json")).unwrap();
    assert_eq!(json["voice"], false);
    let cut = silent.run("cut --silences");
    assert_eq!(cut.code, 1, "{}", cut.stdout);
    assert!(cut.stderr.contains("в записи нет голоса"), "{}", cut.stderr);
    assert_eq!(silent.ok("info"), info, "nothing is cut");
    let listed = silent.run("silences");
    assert!(
        listed.stderr.contains("а не паузы речи"),
        "{}",
        listed.stderr
    );
}
