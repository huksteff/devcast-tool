//! `devcast record` and `devcast ls sources` without recording anything: arguments, dry-run,
//! the control channel and the printed text.

use std::time::Duration;

use dc_capture::audio::{AudioSummary, MicrophoneInfo, TimelineStats};
use dc_capture::input::InputSummary;
use dc_capture::win::{RecordedProject, RecordingSummary, VideoSummary};
use dc_capture::{
    Area, Fps, HostTime, Manifest, MonitorInfo, Pause, RecordingConfig, Source, SourceList, Track,
    WindowInfo,
};
use dc_project::ProjectDir;

use crate::record::summary;
use crate::record_args::{RecordArgs, Switch, config, project_folder};
use crate::record_control::{self, Control, Reply, Server};
use crate::sources;
use crate::test_util::Fixture;

#[test]
fn arguments_make_the_recording_settings() {
    let args = RecordArgs {
        source: Some("area:-1700,150,1280,720".to_owned()),
        fps: Some(120),
        audio: Some(Switch::Off),
        mic: Some("Микрофон (Realtek Audio)".to_owned()),
        ..RecordArgs::default()
    };
    let expected = RecordingConfig {
        fps: Fps::Fps120,
        system_audio: false,
        microphone: Some("Микрофон (Realtek Audio)".to_owned()),
        ..RecordingConfig::new(Source::Area(Area::new(-1700, 150, 1280, 720).unwrap()))
    };
    assert_eq!(config(&args).unwrap(), expected);
    let plain = RecordArgs {
        source: Some("screen:2".to_owned()),
        ..RecordArgs::default()
    };
    assert_eq!(
        config(&plain).unwrap(),
        RecordingConfig::new(Source::Monitor(2))
    );
}

#[test]
fn wrong_arguments_exit_with_2_and_a_hint() {
    let f = Fixture::new();
    for (line, hint) in [
        ("record", "--source screen:1"),
        ("record --source monitor:1", "screen:<номер>"),
        ("record --source screen:0", "нумеруются с 1"),
        ("record --source screen:1 --fps 25", "30, 60 или 120"),
        ("record --source screen:1 --duration 0", "больше 0"),
        ("record --source screen:1 --audio maybe", "devcast help"),
        ("record --source screen:1 -o урок", "*.dcproj"),
        ("record stop --source screen:1", "devcast record stop"),
        ("record --source screen:1 --project a.dcproj", "-o"),
        ("ls sources --from 1", "devcast ls sources"),
    ] {
        let out = f.run(line);
        assert_eq!(out.code, 2, "{line}: {out:?}");
        assert!(out.stderr.contains(hint), "{line}: {}", out.stderr);
        assert!(out.stdout.is_empty(), "{line}");
    }
}

#[test]
fn dry_run_records_nothing_and_names_the_project() {
    let f = Fixture::new();
    let cwd = f.dir.root().parent().unwrap();
    let out = f.run("record --source screen:1 --fps 30 --audio off --dry-run");
    assert_eq!(out.code, 0, "{}", out.stderr);
    let root = cwd.join("recording-1.dcproj");
    assert_eq!(
        out.stdout,
        format!(
            "dry-run, запись не начата: screen:1 30 fps → {}\n",
            root.display()
        )
    );
    assert!(!root.exists());
    std::fs::create_dir(&root).unwrap();
    assert_eq!(
        project_folder(cwd, None).unwrap(),
        cwd.join("recording-2.dcproj")
    );
    let error = project_folder(cwd, Some("recording-1.dcproj".as_ref())).unwrap_err();
    assert_eq!(error.code(), 1, "{error}");
    let out = f.run("record pause --dry-run --json");
    let json: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"dry_run": true, "control": "pause"})
    );
}

/// The only test that uses the real channel `devcast-record-<user>`, so that nothing else in
/// the run takes it at the same time.
#[test]
fn control_reaches_the_recording_process_and_fails_without_one() {
    let f = Fixture::new();
    let out = f.run("record stop");
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("запись не идёт"), "{}", out.stderr);

    let server = Server::start().unwrap();
    assert!(
        Server::start().is_err(),
        "a second recording must not start"
    );
    let caller = std::thread::spawn(|| record_control::send(Control::Pause, false));
    let request = server
        .requests
        .recv_timeout(Duration::from_secs(10))
        .unwrap();
    assert_eq!(request.control, Control::Pause);
    let reply = Reply::Done {
        text: "пауза: записано 0:05.000".to_owned(),
        json: serde_json::json!({"state": "пауза"}),
    };
    request.reply.send(reply).unwrap();
    let answer = caller.join().unwrap().unwrap();
    assert_eq!(answer.text, "пауза: записано 0:05.000");
    assert!(server.close().unwrap().is_empty());

    let out = f.run("record resume");
    assert_eq!(out.code, 1, "the channel is free again: {out:?}");
}

#[test]
fn sources_are_listed_one_per_line() {
    let list = SourceList {
        monitors: vec![
            MonitorInfo {
                index: 1,
                name: Some("DELL U2720Q".to_owned()),
                bounds: Area::new(0, 0, 2560, 1440).unwrap(),
                primary: true,
            },
            MonitorInfo {
                index: 2,
                name: None,
                bounds: Area::new(-1920, 11, 1920, 1080).unwrap(),
                primary: false,
            },
        ],
        windows: vec![WindowInfo {
            id: 263_456,
            title: "Урок 2 — Visual Studio Code".to_owned(),
            app: Some("Code".to_owned()),
            minimized: true,
        }],
    };
    let mics = [MicrophoneInfo {
        name: "Микрофон (Realtek Audio)".to_owned(),
        default: true,
    }];
    let answer = sources::answer(&list, &mics);
    assert_eq!(
        answer.text,
        "screen:1  2560x1440 в 0,0  основной  DELL U2720Q
screen:2  1920x1080 в -1920,11
window:263456  Code — Урок 2 — Visual Studio Code  (свёрнуто: разверни перед записью)
--mic \"Микрофон (Realtek Audio)\"  по умолчанию"
    );
    assert_eq!(answer.json["monitors"][1]["source"], "screen:2");
    assert_eq!(answer.json["windows"][0]["source"], "window:263456");
    assert_eq!(answer.json["microphones"][0]["default"], true);
}

#[test]
fn real_sources_list_has_the_monitors() {
    let f = Fixture::new();
    let out = f.run("ls sources --json");
    assert_eq!(out.code, 0, "{}", out.stderr);
    let json: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    let monitors = json["monitors"].as_array().unwrap();
    for (n, monitor) in monitors.iter().enumerate() {
        assert_eq!(monitor["source"], format!("screen:{}", n + 1));
    }
}

#[test]
fn stop_prints_one_line_about_the_project() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = ProjectDir::create(tmp.path().join("Урок 3.dcproj")).unwrap();
    let start = HostTime::from_micros(1_000_000);
    let mut manifest = Manifest::new(vec![
        Track::new("video", 60, start),
        Track::new("system", 48_000, start),
        Track::new("mic", 48_000, start),
    ]);
    manifest.pauses.push(Pause {
        at: HostTime::from_micros(3_000_000),
        resumed: Some(HostTime::from_micros(4_000_000)),
    });
    for track in &mut manifest.tracks {
        track.file = Some(track.final_file());
    }
    manifest.tracks[0].length = 741;
    manifest.tracks[0].size = Some((1920, 1080));
    let mut project = dc_core::Project::default();
    dc_capture::project::add_recording(&mut project, "recording-1", &manifest).unwrap();
    let audio = |track| AudioSummary {
        track,
        device: "Динамики".to_owned(),
        frames: 592_800,
        stats: TimelineStats::default(),
        reconnects: 0,
        problems: Vec::new(),
    };
    let recorded = RecordedProject {
        dir,
        project,
        summary: RecordingSummary {
            video: VideoSummary {
                frames: 700,
                end: 741,
                lead: None,
                leads: Vec::new(),
            },
            audio: vec![audio("system"), audio("mic")],
            input: InputSummary::default(),
            files: Vec::new(),
            manifest,
        },
    };
    let answer = summary(&recorded).unwrap();
    assert_eq!(
        answer.text,
        format!(
            "записано: {} — 0:12.350, 1920x1080, 60 fps, звук: system, mic, пауз: 1",
            recorded.dir.root().display()
        )
    );
    assert_eq!(answer.json["duration"], 12.35);
    assert_eq!(answer.json["sound"], serde_json::json!(["system", "mic"]));
}
