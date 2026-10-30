//! Task 4.18: «вырежи паузы и оговорки» as an agent does it, on synthesized speech with its
//! recognized words in the cache: find the pauses, cut them leaving a margin, read the words,
//! cut the unwanted ones, check the result. The whole session is one snapshot.

use serde_json::Value;

use crate::test_util::info_duration as duration;
use crate::transcript_tests::Speech;

/// Left of a pause on each side, so speech does not run together (SKILL.md).
const MARGIN: f64 = 0.15;

#[test]
fn cut_pauses_and_a_slip_of_the_tongue() {
    let speech = Speech::new();
    let mut log = String::new();
    let mut calls = 0;
    let mut call = |line: &str| -> String {
        calls += 1;
        let out = speech.ok(line);
        log.push_str(&format!("$ devcast {line}\n{out}\n"));
        out
    };
    let before = duration(&call("info"));
    // Pauses inside the speech: those with sound on both sides.
    let found: Value = serde_json::from_str(&call("silences --json")).unwrap();
    let ranges: Vec<String> = found["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["from"].as_f64().unwrap(), p["to"].as_f64().unwrap()))
        .filter(|(from, to)| *from > 0.0 && *to < before - 0.05)
        .map(|(from, to)| format!("{:.3}-{:.3}", from + MARGIN, to - MARGIN))
        .collect();
    assert!(!ranges.is_empty(), "{log}");
    call(&format!("cut {} --dry-run", ranges.join(",")));
    call(&format!("cut {}", ranges.join(",")));
    // The slip: «Привет!» is not wanted.
    call("transcript --words --to 1");
    call("cut w1");
    let text = call("transcript");
    let after = duration(&call("info"));

    insta::assert_snapshot!(log);
    assert!(!text.contains("Привет"), "{text}");
    for kept in ["Сегодня", "паузы.", "Это занимает пару минут."] {
        assert!(text.contains(kept), "{text}");
    }
    let removed = before - after;
    assert!((2.0..4.0).contains(&removed), "{before} → {after}");
    assert!(calls <= 8, "{calls} calls");
}
