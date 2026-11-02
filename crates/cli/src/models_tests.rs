//! `devcast models`: the models folder of the tests' settings folder; nothing goes to the
//! network (the download itself is tested in dc-models on a local server).

use dc_models::Progress;

use super::progress_line;
use crate::fake_editor::{channel, run};
use crate::test_util::{Fixture, defaults_file};

#[test]
fn ls_and_a_model_already_there() {
    let f = Fixture::new();
    let dead = channel("models");
    let models = defaults_file(f._tmp.path()).with_file_name("models");
    let out = run(&f, "models ls", &dead);
    let fast = models.join("ggml-small.bin");
    assert_eq!(
        out.stdout.lines().next().unwrap(),
        format!("fast      не скачана, ≈ 488 МБ  {}", fast.display())
    );
    std::fs::create_dir_all(&models).unwrap();
    std::fs::write(&fast, vec![0; 2_000_000]).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&run(&f, "models ls --json", &dead).stdout).unwrap();
    assert_eq!(json[0]["ready"], true);
    assert_eq!(json[0]["size"], 2_000_000);
    assert_eq!(json[1]["ready"], false);
    let out = run(&f, "models download fast", &dead);
    assert_eq!(
        out.stdout,
        format!("fast уже скачана: {}\n", fast.display())
    );
    let out = run(&f, "models download accurate --dry-run", &dead);
    assert!(out.stdout.starts_with("dry-run"), "{}", out.stdout);
    assert!(!models.join("ggml-large-v3-turbo.bin.part").exists());
    assert_eq!(run(&f, "models download best", &dead).code, 2);
}

#[test]
fn progress_goes_in_steps_of_ten_percent() {
    let mut shown = None;
    let mut lines = Vec::new();
    for done in [0, 50, 120, 130, 999, 1000] {
        let progress = Progress {
            done: done * 1_000_000,
            total: Some(1_000_000_000),
        };
        lines.extend(progress_line("fast", progress, &mut shown));
    }
    assert_eq!(
        lines,
        [
            "fast: 0% (0 из 1000 МБ)",
            "fast: 10% (120 из 1000 МБ)",
            "fast: 90% (999 из 1000 МБ)",
            "fast: 100% (1000 из 1000 МБ)",
        ]
    );
    let unknown = Progress {
        done: 5,
        total: None,
    };
    assert_eq!(progress_line("fast", unknown, &mut shown), None);
}
