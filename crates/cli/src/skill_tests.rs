use crate::skill::{SKILL, install};
use crate::test_util::Recordings;

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
