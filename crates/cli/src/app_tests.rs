//! Commands to the running app (owner's rule 2026-11-02) with the fake app of `fake_editor.rs`:
//! its recording, its export queue next to the queue of `devcast export`, the captions of its
//! editor, `app status|show|quit`; and the same commands without the app.

use crate::fake_editor::{FakeEditor, run};
use crate::test_util::Fixture;

#[test]
fn recording_of_the_app() {
    let f = Fixture::new();
    let app = FakeEditor::start(&f, "record");
    let ch = app.channel.clone();
    let started = run(&f, "record --app --json", &ch);
    assert_eq!(started.code, 0, "{}", started.stderr);
    let json: serde_json::Value = serde_json::from_str(&started.stdout).unwrap();
    assert_eq!(
        (json["state"].as_str(), json["app"].as_bool()),
        (Some("recording"), Some(true))
    );
    assert!(
        json["project"]
            .as_str()
            .unwrap()
            .ends_with("Запись 1.devcast")
    );
    let again = run(&f, "record --app", &ch);
    assert!(again.stderr.contains("уже записывает"), "{}", again.stderr);
    assert_eq!(
        run(&f, "record pause", &ch).stdout,
        "пауза: записано 0:01.500\n"
    );
    let twice = run(&f, "record pause --app", &ch);
    assert!(
        twice.stderr.contains("devcast record resume"),
        "{}",
        twice.stderr
    );
    assert_eq!(
        run(&f, "record resume", &ch).stdout,
        "продолжено: записано 0:01.500\n"
    );
    let status = run(&f, "app status", &ch).stdout;
    assert!(status.starts_with("запись: идёт "), "{status}");
    let stopped = run(&f, "record stop", &ch);
    assert!(
        stopped.stdout.starts_with("записано: "),
        "{}",
        stopped.stdout
    );
    assert!(stopped.stdout.contains("в редакторе"), "{}", stopped.stdout);
    let idle = run(&f, "record cancel", &ch);
    assert_eq!(idle.code, 1);
    assert!(idle.stderr.contains("запись не идёт"), "{}", idle.stderr);
    app.stop();
}

#[test]
fn record_app_takes_the_settings_of_the_app() {
    let f = Fixture::new();
    let out = f.run("record --app --fps 30");
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("devcast config set"), "{}", out.stderr);
    let dry = f.run("record --app --dry-run");
    assert_eq!(dry.code, 0, "{}", dry.stderr);
    assert!(
        dry.stdout
            .starts_with("dry-run, запись не начата: программа запишет screen:1 60 fps"),
        "{}",
        dry.stdout
    );
    let none = f.run("record --app");
    assert!(
        none.stderr.contains("DevCast не запущен"),
        "{}",
        none.stderr
    );
    let stop = f.run("record stop");
    assert!(stop.stderr.contains("запись не идёт"), "{}", stop.stderr);
}

/// `e…` — the queue of `devcast export`, `a…` — the app's; an export of the project the app
/// holds joins the app's queue.
#[test]
fn exports_of_both_queues() {
    let f = Fixture::new();
    let app = FakeEditor::start(&f, "exports");
    let ch = app.channel.clone();
    let status = run(&f, "export status", &ch);
    assert_eq!(status.stdout, "p1  в очереди  очередь.mp4  (программа)\n");
    let json = run(&f, "export status --json", &ch).stdout;
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["jobs"][0]["tag"], "p1");
    assert_eq!(json["jobs"][0]["queue"], "app");
    let wrong = run(&f, "export cancel e1", &ch);
    assert!(wrong.stderr.contains("e1 не идёт"), "{}", wrong.stderr);
    assert_eq!(run(&f, "export cancel p1", &ch).stdout, "отменено: p1\n");
    let exported = run(&f, "export -o урок.mp4", &ch);
    assert_eq!(exported.code, 0, "{}", exported.stderr);
    assert!(
        exported.stdout.starts_with("p2 готово: "),
        "{}",
        exported.stdout
    );
    let all = run(&f, "app status --json", &ch).stdout;
    let all: serde_json::Value = serde_json::from_str(&all).unwrap();
    assert_eq!(all["exports"][1]["tag"], "p2");
    assert_eq!(all["running"], true);
    app.stop();
}

#[test]
fn captions_stop_reaches_the_editor_only() {
    let f = Fixture::new();
    let alone = f.run("captions stop");
    assert_eq!(alone.code, 1);
    assert!(
        alone.stderr.contains("не открыт в редакторе"),
        "{}",
        alone.stderr
    );
    let app = FakeEditor::start(&f, "captions-stop");
    let out = run(&f, "captions stop", &app.channel);
    assert_eq!(out.stdout, "распознавание субтитров в редакторе не шло\n");
    let batch = run(&f, "batch *.devcast -- captions stop", &app.channel);
    assert_eq!(batch.code, 2, "{}", batch.stderr);
    app.stop();
}

#[test]
fn show_and_quit_the_app() {
    let f = Fixture::new();
    assert!(f.run("app show").stderr.contains("DevCast не запущен"));
    assert_eq!(f.run("app quit").stdout, "DevCast не запущен\n");
    assert_eq!(f.run("app status").stdout, "DevCast не запущен\n");
    let app = FakeEditor::start(&f, "show");
    assert_eq!(
        run(&f, "app show", &app.channel).stdout,
        "HUD программы показан\n"
    );
    app.gone();
    let app = FakeEditor::start(&f, "quit");
    let dry = run(&f, "app quit --dry-run", &app.channel);
    assert_eq!(dry.stdout, "dry-run: программа не закрыта; записи нет\n");
    assert_eq!(run(&f, "app quit", &app.channel).stdout, "DevCast закрыт\n");
    app.gone();
}

#[test]
fn feedback_lists_contacts_and_the_logs_folder() {
    let f = Fixture::new();
    let text = f.run("app feedback");
    assert_eq!(text.code, 0, "{}", text.stderr);
    assert!(text.stdout.starts_with("Нашли баг или есть идея"));
    assert!(text.stdout.contains("Репозиторий: https://"));
    let json: serde_json::Value =
        serde_json::from_str(&f.run("app feedback --json").stdout).unwrap();
    assert!(!json["contacts"].as_array().unwrap().is_empty());
    let logs = std::path::PathBuf::from(json["logs"].as_str().unwrap());
    assert!(logs.ends_with("logs") && !logs.starts_with(std::env::var("APPDATA").unwrap()));
}
