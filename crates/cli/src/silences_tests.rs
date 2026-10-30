//! `devcast silences` on a copy of the owner's speech recording (pauses marked by hand in
//! `dc-audio` tests/silence.rs).

use std::path::Path;

use crate::registry::registry;
use crate::run::Outcome;
use crate::test_util::run_line;

struct Speech {
    _tmp: tempfile::TempDir,
    root: std::path::PathBuf,
}

impl Speech {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Речь с паузами.dcproj");
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings/speech.dcproj");
        crate::test_util::copy_dir(&fixture, &root);
        Self { _tmp: tmp, root }
    }

    fn run(&self, line: &str) -> Outcome {
        run_line(line, &self.root, &registry().unwrap())
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
    // The system track has its own sound in the first 10.65 s.
    assert!(system.contains("0:10.650–0:20.016"), "{system}");
}
