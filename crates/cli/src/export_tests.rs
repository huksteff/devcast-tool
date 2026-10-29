//! `devcast export`: arguments, dry-run, the queue between calls on a channel of the test's
//! own (the real `devcast-export-<user>` is not touched), status and cancel.

use std::path::Path;

use dc_ops::Registry;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::export::{self, Action};
use crate::export_args::ExportArgs;
use crate::globals::Globals;
use crate::run::Answer;
use crate::test_util::{Fixture, ffmpeg};

fn channel(name: &str) -> String {
    format!("test-export-{name}-{}", std::process::id())
}

fn call(
    cwd: &Path,
    registry: &Registry,
    action: Option<Action>,
    id: Option<&str>,
    args: &ExportArgs,
    channel: &str,
) -> Result<Answer, CliError> {
    let globals = Globals::default();
    let ctx = Ctx {
        globals: &globals,
        cwd,
        registry,
    };
    export::export(&ctx, action, id, args, channel)
}

#[test]
fn wrong_arguments_exit_with_2_and_a_hint() {
    let f = Fixture::new();
    for (line, hint) in [
        ("export", "-o урок.mp4"),
        ("export -o урок.avi", "--format mp4|gif"),
        ("export -o a.gif --res 4k", "720p, 1080p или source"),
        ("export -o a.gif --quality low", "нет --quality"),
        ("export -o a.mp4 --loop off", "только для GIF"),
        ("export -o a.mp4 --fps 25", "30, 60, 120"),
        ("export -o a.gif --fps 60", "15, 20, 25, 30"),
        ("export status e1", "всю очередь"),
        ("export cancel x7", "e1, e2"),
        ("export status -o a.mp4", "не берут настроек"),
    ] {
        let out = f.run(line);
        assert_eq!(out.code, 2, "{line}: {out:?}");
        assert!(out.stderr.contains(hint), "{line}: {}", out.stderr);
    }
}

#[test]
fn dry_run_shows_the_export_and_writes_nothing() {
    let f = Fixture::new();
    let out = f.run("export -o готовое.gif --fps 20 --loop off --dry-run");
    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stderr, "");
    assert!(
        out.stdout.starts_with("dry-run, экспорт не начат: "),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("готовое.gif (gif 1080p 20 fps один раз)"),
        "{}",
        out.stdout
    );
    assert!(!f.cwd.join("готовое.gif").exists());
    let up = f.run("export -o ../выше.mp4 --dry-run --json");
    let json: serde_json::Value = serde_json::from_str(&up.stdout).unwrap();
    let path = std::path::PathBuf::from(json["out"].as_str().unwrap());
    assert_eq!(path, f.cwd.parent().unwrap().join("выше.mp4"));
}

#[test]
fn status_and_cancel_without_a_queue() {
    let f = Fixture::new();
    let name = channel("idle");
    let none = ExportArgs::default();
    let status = call(
        &f.cwd,
        &f.registry,
        Some(Action::Status),
        None,
        &none,
        &name,
    )
    .unwrap();
    assert_eq!(status.text, "экспорт не идёт");
    assert_eq!(status.json, serde_json::json!({"jobs": []}));
    let Err(error) = call(
        &f.cwd,
        &f.registry,
        Some(Action::Cancel),
        None,
        &none,
        &name,
    ) else {
        panic!("cancel without a queue must fail");
    };
    assert!(error.to_string().contains("отменять нечего"), "{error}");
}

/// Two exports at once: whichever call takes the channel runs the queue, the other waits in it;
/// both files come out.
#[test]
fn two_calls_share_the_queue_and_both_files_come_out() {
    let f = Fixture::new();
    let video = f.cwd.join("клип.mp4");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=320x180:rate=30:d=1.5",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000:d=1.5",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-shortest",
        video.to_str().unwrap(),
    ]);
    let made = f.run("new клип.mp4 -o экспорт.dcproj");
    assert_eq!(made.code, 0, "{made:?}");
    let project = f.cwd.join("экспорт.dcproj");
    let name = channel("two");
    let mp4 = ExportArgs {
        out: Some("готовое видео.mp4".into()),
        res: Some(crate::export_args::Res::Source),
        fps: Some(30),
        ..ExportArgs::default()
    };
    let gif = ExportArgs {
        out: Some("готовое видео.gif".into()),
        res: Some(crate::export_args::Res::Source),
        ..ExportArgs::default()
    };
    let answers = std::thread::scope(|scope| {
        let calls = [&mp4, &gif].map(|args| {
            let (project, registry, name) = (&project, &f.registry, &name);
            scope.spawn(move || call(project, registry, None, None, args, name))
        });
        calls.map(|call| call.join().unwrap().unwrap())
    });
    for (answer, file) in answers
        .iter()
        .zip(["готовое видео.mp4", "готовое видео.gif"])
    {
        println!("{}", answer.text);
        assert!(answer.text.contains("готово: "), "{}", answer.text);
        assert!(answer.text.contains("320×180"), "{}", answer.text);
        assert!(project.join(file).is_file(), "{file}");
    }
    let probe = dc_media::probe(&project.join("готовое видео.mp4")).unwrap();
    assert_eq!(probe.frames, 45);
    assert!(
        probe.streams.len() == 2,
        "video and sound: {:?}",
        probe.streams
    );
    let none = ExportArgs::default();
    let status = call(
        &project,
        &f.registry,
        Some(Action::Status),
        None,
        &none,
        &name,
    )
    .unwrap();
    assert_eq!(
        status.text, "экспорт не идёт",
        "the queue closed when it ran empty"
    );
}
