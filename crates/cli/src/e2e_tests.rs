//! The whole CLI session an agent would run on a two-clip project, as one readable snapshot.

use crate::ls_tests::Fixture;

const PLAN: &str = r#"[
  {"op": "zoom set", "id": "z1", "scale": 2},
  {"op": "crop", "clip": "c2", "rect": "0.1,0,0.8,1"},
  {"op": "aspect", "aspect": "16:9"},
  {"op": "clip move", "id": "c3", "to": 1},
  {"op": "cut", "ranges": "0-1"}
]"#;

const SCENARIO: &[&str] = &[
    "info",
    "ls clips",
    "cut 5-7 --dry-run",
    "cut 5-7",
    "zoom add --at 2 --duration 2",
    "zoom add --at 15 --scale 2 --mode fixed --x 0.3 --y 0.4",
    "ls zooms",
    "clip split --at 10",
    "apply план.json --dry-run",
    "apply план.json",
    "ls clips",
    "zoom ad --at 1",
    "zoom rm z9",
    "undo",
    "redo",
    "history",
    "info",
];

#[test]
fn scenario_output_is_short_and_readable() {
    let f = Fixture::new();
    std::fs::write(f.dir.root().join("план.json"), PLAN).unwrap();
    let mut log = String::new();
    for line in SCENARIO {
        let out = f.run(line);
        log.push_str(&format!("$ devcast {line}\n{}", out.stdout));
        for err in out.stderr.lines() {
            log.push_str(&format!("! {err}\n"));
        }
        log.push_str(&format!("→ {}\n\n", out.code));
    }
    insta::assert_snapshot!(log);
}
