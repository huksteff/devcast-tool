//! Records the v2 fixtures (task 3.16) into `fixtures/recordings/<name>.dcproj` (Git LFS):
//! `clicks`, `typing`, `system_sound`, `fast_cursor`, `window`. Each is a project made by the
//! recorder (`ProjectRecording`, as `devcast record` does) of the fixtures' own window on monitor
//! `DEVCAST_TEST_MONITOR`; synthetic input goes only into that window. What each one holds:
//! `fixtures/recordings/README.md`.
//!
//! Takes the pointer and the keyboard for about a minute and plays tones: run only when nobody
//! works at the computer. All, or the named ones; an existing fixture of that name is replaced:
//! `DEVCAST_TEST_MONITOR=2 cargo run -p dc-capture --example record_fixtures [-- clicks typing …]`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unsafe_code,
    reason = "a fixture script: a failed step must stop it; synthetic input and the window are Win32"
)]

#[path = "../../tests/common/mod.rs"]
mod common;

mod canvas;
mod send;

use std::path::{Path, PathBuf};

use canvas::Canvas;
use dc_capture::input::{CursorEvent, KeyEvent, Modifiers, read_cursor, read_keys};
use dc_capture::win::{ProjectRecording, RecordedProject};
use dc_capture::{Area, RecordingConfig, Source};
use dc_project::Defaults;
use send::{
    ALT, CTRL, DELETE, ENTER, ESC, F5, LEFT, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_WHEEL, PointerBack, SHIFT, act, chord, click, glide, move_to, type_text, wait,
};

const WIDTH: i32 = 1280;
const HEIGHT: i32 = 720;
const ALL: [&str; 5] = ["clicks", "typing", "system_sound", "fast_cursor", "window"];

/// The canvas and its top-left corner on the desktop.
struct Scene {
    canvas: Canvas,
    x: i32,
    y: i32,
}

impl Scene {
    fn at(&self, x: i32, y: i32) -> (i32, i32) {
        (self.x + x, self.y + y)
    }

    fn area(&self) -> Source {
        Source::Area(Area::new(self.x, self.y, WIDTH as u32, HEIGHT as u32).unwrap())
    }
}

fn main() {
    let names: Vec<String> = std::env::args().skip(1).collect();
    for name in &names {
        assert!(ALL.contains(&name.as_str()), "{name}: one of {ALL:?}");
    }
    let monitor = common::test_monitor().bounds;
    let x = monitor.x() + (monitor.width() as i32 - WIDTH) / 2;
    let y = monitor.y() + (monitor.height() as i32 - HEIGHT) / 2;
    let _back = PointerBack::save();
    let scene = Scene {
        canvas: Canvas::show(x, y, WIDTH, HEIGHT),
        x,
        y,
    };
    let (fx, fy) = scene.at(WIDTH / 2, HEIGHT - 60);
    click(fx, fy);
    wait(300);
    assert!(
        scene.canvas.is_foreground(),
        "the fixture window did not get the keyboard: typing would go elsewhere"
    );
    for name in ALL {
        if names.is_empty() || names.iter().any(|n| n == name) {
            scene.canvas.clear();
            wait(300);
            match name {
                "clicks" => clicks(&scene),
                "typing" => typing(&scene),
                "system_sound" => system_sound(&scene),
                "fast_cursor" => fast_cursor(&scene),
                _ => window(&scene),
            }
        }
    }
    println!("готово: проверь кадры и закоммить fixtures/recordings (Git LFS)");
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings")
}

/// Records `source` into `fixtures/recordings/<name>.dcproj` while `actions` run, with a
/// second of stillness before and after.
fn record(name: &str, source: Source, actions: impl FnOnce()) -> (RecordedProject, PathBuf) {
    let root = fixtures().join(format!("{name}.dcproj"));
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    let config = RecordingConfig::new(source);
    let recording =
        ProjectRecording::start(&config, &root, "devcast fixtures", Defaults::factory()).unwrap();
    wait(1000);
    actions();
    wait(1000);
    let recorded = recording.stop().unwrap();
    let media = root.join("media/recording-1");
    let clip = &recorded.project.clips[0].media;
    println!(
        "{name}: {:.2} s, {}x{}, {} кадров, звук {:?}, {:?}",
        clip.duration.as_secs_f64(),
        clip.size.width,
        clip.size.height,
        recorded.summary.video.frames,
        recorded
            .summary
            .audio
            .iter()
            .map(|a| a.track)
            .collect::<Vec<_>>(),
        recorded.summary.input,
    );
    (recorded, media)
}

/// The presses in `cursor.bin` are where they were sent, give or take the rounding of the
/// 0..65535 units of SendInput.
fn check_downs(media: &Path, expected: &[(i32, i32)]) {
    let found = downs(media);
    let near =
        |(a, b): (&(i32, i32), &(i32, i32))| (a.0 - b.0).abs() <= 2 && (a.1 - b.1).abs() <= 2;
    assert!(
        found.len() == expected.len() && found.iter().zip(expected).all(near),
        "presses in pixels of the video: {found:?}, sent {expected:?}"
    );
}

fn downs(media: &Path) -> Vec<(i32, i32)> {
    let events = read_cursor(media).unwrap();
    events
        .iter()
        .filter_map(|e| match e.event {
            CursorEvent::Down { x, y, .. } => Some((x, y)),
            _ => None,
        })
        .collect()
}

/// Six left clicks in different places, a double click, a right click, the wheel.
fn clicks(scene: &Scene) {
    let points = [
        (200, 150),
        (1080, 150),
        (640, 360),
        (200, 600),
        (1080, 600),
        (900, 300),
    ];
    let (_, media) = record("clicks", scene.area(), || {
        let mut from = scene.at(640, 650);
        for &(px, py) in &points {
            let to = scene.at(px, py);
            glide(from, to, 30, 10);
            wait(300);
            click(to.0, to.1);
            wait(700);
            from = to;
        }
        let (dx, dy) = scene.at(400, 400);
        glide(from, (dx, dy), 30, 10);
        click(dx, dy);
        wait(80);
        click(dx, dy);
        wait(700);
        act(dx, dy, MOUSEEVENTF_RIGHTDOWN, 0);
        wait(80);
        act(dx, dy, MOUSEEVENTF_RIGHTUP, 0);
        wait(500);
        for _ in 0..3 {
            act(dx, dy, MOUSEEVENTF_WHEEL, -120);
            wait(150);
        }
    });
    let mut expected: Vec<(i32, i32)> = points.to_vec();
    expected.extend([(400, 400); 3]);
    check_downs(&media, &expected);
}

/// Typed text (never stored) and shortcuts (stored in keys.bin).
fn typing(scene: &Scene) {
    let (_, media) = record("typing", scene.area(), || {
        let (x, y) = scene.at(1000, 500);
        move_to(x, y);
        type_text("hello devcast");
        chord(&[ENTER]);
        type_text("shortcuts");
        for letter in *b"ACVZ" {
            chord(&[CTRL, u16::from(letter)]);
        }
        chord(&[CTRL, SHIFT, u16::from(b'P')]);
        chord(&[ALT, SHIFT, u16::from(b'F')]);
        chord(&[F5]);
        for _ in 0..3 {
            chord(&[LEFT]);
        }
        chord(&[DELETE]);
        chord(&[ESC]);
    });
    assert_eq!(Canvas::text(), "hello devcast\rshortcuts");
    let with = |key: u16, ctrl, shift, alt| KeyEvent {
        key,
        modifiers: Modifiers {
            ctrl,
            shift,
            alt,
            win: false,
        },
    };
    let plain = |key| with(key, false, false, false);
    let ctrl = |letter: u8| with(u16::from(letter), true, false, false);
    let mut expected = vec![plain(ENTER), ctrl(b'A'), ctrl(b'C'), ctrl(b'V'), ctrl(b'Z')];
    expected.push(with(u16::from(b'P'), true, true, false));
    expected.push(with(u16::from(b'F'), false, true, true));
    expected.extend([plain(F5), plain(LEFT), plain(LEFT), plain(LEFT)]);
    expected.extend([plain(DELETE), plain(ESC)]);
    let keys: Vec<KeyEvent> = read_keys(&media).unwrap().iter().map(|k| k.event).collect();
    assert_eq!(keys, expected, "only shortcuts and special keys, no text");
}

/// Two tones through the speakers: 1 kHz, then 440 Hz, a second each.
fn system_sound(scene: &Scene) {
    let (recorded, _) = record("system_sound", scene.area(), || {
        common::tone(1000, 1.0, -12.0).wait().unwrap();
        wait(1000);
        common::tone(440, 1.0, -12.0).wait().unwrap();
    });
    let system = recorded.summary.audio.iter().find(|a| a.track == "system");
    assert!(system.is_some(), "no system sound track");
}

/// Fast circles, flicks across the window and clicks on the move.
fn fast_cursor(scene: &Scene) {
    let (_, media) = record("fast_cursor", scene.area(), || {
        let (cx, cy) = scene.at(WIDTH / 2, HEIGHT / 2);
        for step in 0..=240 {
            let angle = f64::from(step) / 120.0 * std::f64::consts::TAU;
            let x = cx + (250.0 * angle.cos()) as i32;
            let y = cy + (250.0 * angle.sin()) as i32;
            move_to(x, y);
            wait(2);
        }
        let (left, right) = (scene.at(100, 0).0, scene.at(WIDTH - 100, 0).0);
        for flick in 0..6 {
            let (from, to) = if flick % 2 == 0 {
                (left, right)
            } else {
                (right, left)
            };
            let y = scene.at(0, 150 + flick * 80).1;
            glide((from, y), (to, y), 10, 2);
            click(to, y);
        }
    });
    let events = read_cursor(&media).unwrap();
    let moves = events
        .iter()
        .filter(|e| matches!(e.event, CursorEvent::Move { .. }))
        .count();
    assert!(moves > 250, "{moves} moves");
    assert_eq!(downs(&media).len(), 6);
}

/// The window itself as the source, with a few clicks and typed letters.
fn window(scene: &Scene) {
    let (recorded, media) = record("window", Source::Window(scene.canvas.id()), || {
        let mut from = scene.at(640, 650);
        for &(px, py) in &[(300, 200), (980, 200), (640, 500)] {
            let to = scene.at(px, py);
            glide(from, to, 30, 10);
            click(to.0, to.1);
            wait(500);
            from = to;
        }
        type_text("window");
    });
    let size = recorded.project.clips[0].media.size;
    println!("window: frame {}x{}", size.width, size.height);
    check_downs(&media, &[(300, 200), (980, 200), (640, 500)]);
}
