use crate::skill::{SKILL, install};
use crate::test_util::Recordings;

#[test]
fn codex_skill_installs_separately_from_claude_and_supports_dry_run() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("Мой профиль");
    let target = crate::skill::in_home(&home, Some("codex"));
    assert_eq!(target, home.join(".agents/skills/devcast/SKILL.md"));
    let claude = crate::skill::in_home(&home, None);
    assert_eq!(claude, home.join(".claude/skills/devcast/SKILL.md"));
    install(&target, true).unwrap();
    assert!(!target.exists());
    install(&target, false).unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), SKILL);
    assert!(!claude.exists());
}

#[test]
fn skill_install_accepts_codex_without_changing_the_default() {
    let f = crate::test_util::Fixture::new();
    for (args, folder) in [
        ("skill install --agent codex --dry-run", ".agents"),
        ("skill install --dry-run", ".claude"),
    ] {
        let out = f.run(args);
        assert_eq!(out.code, 0, "{}", out.stderr);
        assert!(out.stdout.contains(folder), "{}", out.stdout);
    }
    assert_eq!(f.run("skill install --agent missing --dry-run").code, 2);
    assert_eq!(f.run("skill files --agent codex --dry-run").code, 2);
}

/// Every `devcast …` line of SKILL.md's code blocks runs, in order, on a 30 s project of two
/// recordings with telemetry, a picture `логотип.png`, a sound `музыка.mp3` and a font
/// `шрифт.ttf`; the JSON block is the plan.
#[test]
fn skill_examples_run() {
    let f = Recordings::with_telemetry(&[20.0, 10.0]);
    let size = dc_core::PixelSize {
        width: 4,
        height: 2,
    };
    let logo = dc_media::encode_png(size, &[200; 4 * 2 * 4]).unwrap();
    std::fs::write(f.dir.root().join("логотип.png"), logo).unwrap();
    let music = f.dir.root().join("музыка.mp3");
    crate::test_util::ffmpeg(&["-f", "lavfi", "-i", "sine=d=6", music.to_str().unwrap()]);
    let font = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts/PressStart2P-Regular.ttf");
    std::fs::copy(font, f.dir.root().join("шрифт.ttf")).unwrap();
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

/// Every command of `devcast help` (the clap tree and the registry of the editor) is in
/// SKILL.md as `devcast <команда>` or inline code: a new command reaches the agents' instructions.
#[test]
fn skill_mentions_every_command() {
    let registry = dc_ops::Registry::devcast().unwrap();
    let help = crate::help::help(&[], &registry).unwrap();
    let missing: Vec<String> = help.json["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["name"].as_str().unwrap().to_owned())
        .filter(|name| !mentioned(name))
        .collect();
    assert!(
        missing.is_empty(),
        "в skill/SKILL.md нет команд: {}. Добавь каждую в skill/SKILL.md: пример `devcast <команда> …` или упоминание `<команда>` в тексте, в том же стиле",
        missing.join(", ")
    );
}

/// `devcast <name>` or `` `<name> `` (inline code), followed by a space, a backtick, a quote or the
/// end of the line.
fn mentioned(name: &str) -> bool {
    [format!("devcast {name}"), format!("`{name}")]
        .iter()
        .any(|needle| {
            SKILL.match_indices(needle.as_str()).any(|(at, _)| {
                SKILL[at + needle.len()..]
                    .chars()
                    .next()
                    .is_none_or(|next| matches!(next, ' ' | '`' | '\n' | '\r' | '"' | '»'))
            })
        })
}

/// Every `devcast …` line of the brief of «Сделать красиво» (`dc_agent::prompt::BRIEF`, the short
/// editing part of the skill) runs, in order, on a 30 s project of two recordings.
#[test]
fn brief_examples_run() {
    let f = Recordings::with_telemetry(&[20.0, 10.0]);
    for block in dc_agent::prompt::BRIEF.split("```").skip(1).step_by(2) {
        for line in block.lines().filter_map(|l| l.strip_prefix("devcast ")) {
            let out = f.run(line);
            assert_eq!(out.code, 0, "{line}: {}", out.stderr);
        }
    }
}
