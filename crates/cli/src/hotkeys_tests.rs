//! `devcast hotkeys`: the rules of the settings window, in the file without the app and in the
//! app when it runs.

use dc_settings::Store;

use crate::fake_editor::{FakeEditor, channel, run};
use crate::test_util::{Fixture, defaults_file};

fn stored(f: &Fixture) -> dc_settings::AppSettings {
    let file = defaults_file(f._tmp.path()).with_file_name(dc_settings::FILE);
    Store::load(&file).unwrap().get().clone()
}

#[test]
fn ls_shows_ids_current_and_default() {
    let f = Fixture::new();
    let dead = channel("hotkeys-ls");
    assert_eq!(
        run(&f, "hotkeys set pause --clear", &dead).stdout,
        "pause=нет\n"
    );
    let out = run(&f, "hotkeys ls", &dead);
    let lines: Vec<&str> = out.stdout.lines().collect();
    assert_eq!(
        lines[0],
        "глобальные (из любой программы, нужен Ctrl или Alt):"
    );
    assert_eq!(lines[1], "start_stop       Ctrl+Shift+R");
    assert_eq!(lines[2], "pause            нет (по умолчанию Ctrl+Shift+P)");
    assert_eq!(lines[4], "редактор:");
    assert!(lines.contains(&"save             Ctrl+S"), "{}", out.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&run(&f, "hotkeys ls --json", &dead).stdout).unwrap();
    assert_eq!(
        json[1],
        serde_json::json!({"id": "pause", "scope": "global", "keys": null, "default": "Ctrl+Shift+P"})
    );
}

#[test]
fn a_taken_shortcut_is_refused_or_swapped() {
    let f = Fixture::new();
    let dead = channel("hotkeys-swap");
    let out = run(&f, "hotkeys set blade_tool ctrl+s", &dead);
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("Ctrl+S уже у save"), "{}", out.stderr);
    assert!(out.stderr.contains("--swap"), "{}", out.stderr);
    let out = run(&f, "hotkeys set blade_tool ctrl+s --swap", &dead);
    assert_eq!(out.stdout, "blade_tool=Ctrl+S\nsave=B\n");
    let out = run(&f, "hotkeys set start_stop F9", &dead);
    assert!(out.stderr.contains("нужен Ctrl или Alt"), "{}", out.stderr);
    let out = run(&f, "hotkeys set start_stop Win+R", &dead);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("Win нельзя"), "{}", out.stderr);
    let out = run(&f, "hotkeys set stop Ctrl+R", &dead);
    assert!(out.stderr.contains("devcast hotkeys ls"), "{}", out.stderr);
    let out = run(&f, "hotkeys reset --all", &dead);
    assert_eq!(out.stdout, "blade_tool=B\nsave=Ctrl+S\n");
    assert_eq!(stored(&f), dc_settings::AppSettings::default());
    assert_eq!(run(&f, "hotkeys reset", &dead).code, 2);
}

#[test]
fn the_running_app_takes_the_shortcut() {
    let f = Fixture::new();
    let editor = FakeEditor::start(&f, "hotkeys-app");
    let out = run(&f, "hotkeys set start_stop Ctrl+Alt+R", &editor.channel);
    assert_eq!(
        out.stdout,
        "start_stop=Ctrl+Alt+R\nпрограмма DevCast применила сразу\n"
    );
    let out = run(&f, "hotkeys set blade_tool Ctrl+Alt+R", &editor.channel);
    assert!(out.stderr.contains("уже у start_stop"), "{}", out.stderr);
    editor.stop();
    assert_eq!(stored(&f).hotkeys.start_stop.as_deref(), Some("Ctrl+Alt+R"));
}
