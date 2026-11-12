//! `devcast export`: arguments, dry-run, the queue between calls on a channel of the test's
//! own (the real `devcast-export-<user>` is not touched), status and cancel.

use std::path::Path;

use dc_ops::Registry;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::export::{self, Action};
use crate::export_args::ExportArgs;
use crate::globals::Globals;
use crate::run::{Answer, Local};
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
        local: Local {
            app_channel: Some(crate::test_util::NO_APP),
            ..Local::default()
        },
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
        ("export -o a.mp4 --from later", "--from"),
        ("export -o a.mp4 --from 1 --to 0.5", "внутри видео"),
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
            .contains("готовое.gif (gif 720p 20 fps один раз)"),
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
    let made = f.run("new клип.mp4 -o экспорт.devcast");
    assert_eq!(made.code, 0, "{made:?}");
    let project = f.cwd.join("экспорт.devcast");
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
        from: Some("0.5".to_owned()),
        to: Some("1".to_owned()),
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
    // `--from 0.5 --to 1`: the piece only, and the project stays whole.
    assert!(answers[1].text.contains(" 0.5 с, "), "{}", answers[1].text);
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

/// One output moment of `file` (frame `n`) as 32 × 18 gray pixels.
fn small_frame(file: &Path, n: u32) -> Vec<u8> {
    let filter = format!(r"select=eq(n\,{n}),scale=32:18,format=gray");
    let file = file.to_str().unwrap();
    let args = [
        "-i",
        file,
        "-vf",
        &filter,
        "-frames:v",
        "1",
        "-f",
        "rawvideo",
        "-",
    ];
    crate::test_util::decoded(&args)
}

#[test]
fn a_piece_shows_the_frames_of_the_whole_video_there() {
    let f = Fixture::new();
    let project = f.cwd.join("клики.devcast");
    crate::test_util::fixture_copy("clicks.devcast", &project);
    // The zoom of the clicks goes on past the end of the piece: its camera must stay as in the
    // whole video, not end with the piece.
    let registry = Registry::devcast().unwrap();
    let zoom = crate::test_util::run_line("zoom add --at clicks", &project, &registry);
    assert_eq!(zoom.code, 0, "{zoom:?}");
    let name = channel("piece");
    let args = |out: &str, from: Option<&str>, to: Option<&str>| ExportArgs {
        out: Some(out.into()),
        res: Some(crate::export_args::Res::Source),
        fps: Some(30),
        from: from.map(str::to_owned),
        to: to.map(str::to_owned),
        ..ExportArgs::default()
    };
    for args in [
        args("целое.mp4", None, None),
        args("кусок.mp4", Some("1"), Some("4")),
    ] {
        call(&project, &registry, None, None, &args, &name).unwrap();
    }
    // Output 3 s: frame 90 of the whole video, frame 60 of the piece from 1 s.
    let whole = small_frame(&project.join("целое.mp4"), 90);
    let piece = small_frame(&project.join("кусок.mp4"), 60);
    let differ = whole.iter().zip(&piece).map(|(a, b)| a.abs_diff(*b)).max();
    assert!(differ.is_some_and(|d| d <= 8), "{differ:?}");
}
