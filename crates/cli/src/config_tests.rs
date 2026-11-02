//! `devcast config`: the file of the tests' settings folder (`Настройки DevCast` next to the
//! project, a path with Cyrillic and spaces) when no app answers, the app when one does.

use dc_capture::{Fps, Source};
use dc_settings::{Countdown, Language, Store};

use crate::fake_editor::{FakeEditor, channel, run};
use crate::test_util::{Fixture, defaults_file};

fn file(f: &Fixture) -> std::path::PathBuf {
    defaults_file(f._tmp.path()).with_file_name(dc_settings::FILE)
}

fn stored(f: &Fixture) -> dc_settings::AppSettings {
    Store::load(&file(f)).unwrap().get().clone()
}

#[test]
fn show_lists_every_key_and_writes_nothing() {
    let f = Fixture::new();
    let out = run(&f, "config show", &channel("config-show"));
    assert_eq!(out.code, 0, "{}", out.stderr);
    let lines: Vec<&str> = out.stdout.lines().collect();
    assert_eq!(lines.len(), dc_settings::Key::ALL.len());
    assert!(lines[0].starts_with("recordings_dir="), "{}", lines[0]);
    assert_eq!(
        &lines[1..],
        [
            "recording.source=screen:1",
            "recording.fps=60",
            "recording.mic=off",
            "recording.audio=on",
            "recording.countdown=3",
            "hud.hide=on",
            "updates.auto=on",
            "updates.beta=off",
            "captions.model=fast",
            "captions.language=auto",
            "captions.tracks=mic",
        ]
    );
    assert!(!file(&f).exists());
}

#[test]
fn set_without_the_app_writes_the_file() {
    let f = Fixture::new();
    let dead = channel("config-file");
    let line = "config set recording.fps=120 recording.countdown=10 captions.language=ru recording.source=area:-1700,150,1280,720 recordings_dir=Записи";
    let out = run(&f, line, &dead);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let recordings = std::path::absolute(f.cwd.join("Записи")).unwrap();
    assert_eq!(
        out.stdout,
        format!(
            "recordings_dir=\"{}\"\nrecording.source=area:-1700,150,1280,720\nrecording.fps=120\nrecording.countdown=10\ncaptions.language=ru\n",
            recordings.display()
        )
    );
    let settings = stored(&f);
    assert_eq!(settings.recording.fps, Fps::Fps120);
    assert_eq!(settings.countdown, Countdown::Ten);
    assert_eq!(settings.speech_language, Language::Russian);
    assert_eq!(settings.recordings_dir, recordings);
    let again = run(&f, "config set recording.fps=120", &dead);
    assert_eq!(again.stdout, "без изменений\n");
    let mic = ["config", "set", "recording.mic=Микрофон (Rode NT-USB)"];
    let args = mic.iter().map(|a| (*a).to_owned()).collect();
    let settings_dir = file(&f).parent().unwrap().to_path_buf();
    let local = crate::Local {
        settings: Some(&settings_dir),
        app_channel: Some(&dead),
        ..crate::Local::default()
    };
    let out = crate::run_with(args, &f.cwd, &f.registry, local);
    assert_eq!(out.stdout, "recording.mic=\"Микрофон (Rode NT-USB)\"\n");
    assert_eq!(
        stored(&f).recording.microphone.as_deref(),
        Some("Микрофон (Rode NT-USB)")
    );
}

#[test]
fn a_wrong_key_or_value_changes_nothing_and_says_what_is_valid() {
    let f = Fixture::new();
    let dead = channel("config-wrong");
    let out = run(&f, "config set recording.fps=60 recording.speed=2", &dead);
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.contains("ключа «recording.speed» нет"),
        "{}",
        out.stderr
    );
    assert!(out.stderr.contains("captions.tracks"), "{}", out.stderr);
    let out = run(&f, "config set recording.fps=45", &dead);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("30, 60 или 120"), "{}", out.stderr);
    let out = run(&f, "config set recording.source=monitor", &dead);
    assert!(out.stderr.contains("devcast ls sources"), "{}", out.stderr);
    assert!(!file(&f).exists());
}

#[test]
fn reset_and_dry_run() {
    let f = Fixture::new();
    let dead = channel("config-reset");
    run(&f, "config set recording.fps=30 hud.hide=off", &dead);
    let out = run(&f, "config reset recording.fps --dry-run", &dead);
    assert_eq!(
        out.stdout,
        "dry-run, настройки не изменены: recording.fps=60\n"
    );
    assert_eq!(stored(&f).recording.fps, Fps::Fps30);
    let out = run(&f, "config reset", &dead);
    assert_eq!(out.stdout, "recording.fps=60\nhud.hide=on\n");
    assert_eq!(stored(&f), dc_settings::AppSettings::default());
}

#[test]
fn the_running_app_applies_the_edit() {
    let f = Fixture::new();
    let editor = FakeEditor::start(&f, "config-app");
    let out = run(
        &f,
        "config set recording.source=screen:2 --json",
        &editor.channel,
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    let json: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(json["app"], true);
    assert_eq!(json["changed"]["recording.source"], "screen:2");
    let text = run(&f, "config set captions.model=accurate", &editor.channel);
    assert_eq!(
        text.stdout,
        "captions.model=accurate\nпрограмма DevCast применила сразу\n"
    );
    editor.stop();
    assert_eq!(stored(&f).recording.source, Source::Monitor(2));
}
