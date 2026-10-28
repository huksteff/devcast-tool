use crate::ls_tests::Fixture;
use crate::skill::{SKILL, install};

/// Every `devcast …` line of SKILL.md's code blocks runs, in order, on a 30 s two-clip project;
/// the JSON block is the plan.
#[test]
fn skill_examples_run() {
    let f = Fixture::new();
    let mut transcript = String::new();
    for block in SKILL.split("```").skip(1).step_by(2) {
        if let Some(plan) = block.strip_prefix("json") {
            std::fs::write(f.dir.root().join("plan.json"), plan).unwrap();
            continue;
        }
        for line in block.lines().filter_map(|l| l.strip_prefix("devcast ")) {
            let line = line.split(" #").next().unwrap().trim_end();
            let out = f.run(line);
            assert_eq!(out.code, 0, "{line}: {}", out.stderr);
            transcript.push_str(&format!("$ {line}\n{}", out.stdout));
        }
    }
    assert!(transcript.contains("$ apply plan.json\n"), "{transcript}");
}

#[test]
fn install_writes_skill_and_dry_run_does_not() {
    let home = tempfile::tempdir().unwrap();
    let target = home
        .path()
        .join("Мой профиль")
        .join(".claude/skills/devcast/SKILL.md");
    let answer = install(&target, true).unwrap();
    assert!(answer.text.starts_with("dry-run"), "{}", answer.text);
    assert!(!target.exists());
    let answer = install(&target, false).unwrap();
    assert_eq!(
        answer.text,
        format!("skill установлен: {}", target.display())
    );
    assert_eq!(std::fs::read_to_string(&target).unwrap(), SKILL);
    install(&target, false).unwrap();
}
