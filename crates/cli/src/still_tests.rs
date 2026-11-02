//! `devcast frame` and `devcast sheet` on a copy of the fixture `clicks.devcast` (a white screen
//! with a cursor) with annotations: what the evaluator shows reaches the PNG, the grid lies on
//! top, the sheet has its frames in rows. Real GPU or WARP, like the export tests.

use std::path::{Path, PathBuf};

use dc_media::{Picture, decode_picture};
use dc_render::SHEET_GAP;

use crate::run::{Local, run_with};
use crate::test_util::{decoded, defaults_file, fixture_copy, run_line_with};

struct Clicks {
    tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Clicks {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Кадры урока.devcast");
        fixture_copy("clicks.devcast", &root);
        let sticker =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/images/sticker.png");
        std::fs::copy(sticker, root.join("наклейка.png")).unwrap();
        Self { tmp, root }
    }

    fn ok(&self, line: &str) -> String {
        let defaults = defaults_file(self.tmp.path());
        let channel = format!("test-export-still-{}", std::process::id());
        let local = Local {
            defaults: Some(&defaults),
            export_channel: Some(&channel),
            ..Local::default()
        };
        let args = line.split(' ').map(str::to_owned).collect();
        let out = run_with(
            args,
            &self.root,
            &dc_ops::Registry::devcast().unwrap(),
            local,
        );
        assert_eq!(out.code, 0, "{line}: {}", out.stderr);
        out.stdout.trim_end().to_owned()
    }

    fn png(&self, name: &str) -> Picture {
        decode_picture(&self.tmp.path().join(name)).unwrap()
    }
}

fn pixel(picture: &Picture, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * picture.size.width + x) * 4) as usize;
    let rgba = &picture.frames[0].rgba;
    [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
}

#[test]
fn frame_draws_the_annotations_and_the_grid() {
    let f = Clicks::new();
    f.ok("annot add shape --at 1 --duration 4 --rect 0.6,0.3,0.3,0.3");
    f.ok("shape set a1 --figure rectangle --fill #ef4444ff --outline false");
    f.ok("annot add image --at 1 --duration 4 --rect 0.1,0.55,0.25,0.3");
    f.ok("image set a2 --file наклейка.png");
    let text = f.ok("frame --at 2 -o ../кадр.png");
    assert!(text.starts_with("кадр 0:02.000 (1280×720) → "), "{text}");
    assert!(text.ends_with("кадр.png") && !text.contains(".."), "{text}");
    let plain = f.png("кадр.png");
    assert_eq!((plain.size.width, plain.size.height), (1280, 720));
    assert_eq!(
        pixel(&plain, 960, 324),
        [0xef, 0x44, 0x44, 0xff],
        "the shape"
    );
    assert_ne!(pixel(&plain, 288, 504)[..3], [0xff; 3], "the picture");
    assert_eq!(pixel(&plain, 960, 650), [0xff; 4], "the screen");

    f.ok("frame --at 2 --grid -o ../сетка.png");
    let grid = f.png("сетка.png");
    let column = |p: &Picture| (630..650).map(|x| pixel(p, x, 650)).collect::<Vec<_>>();
    assert_ne!(column(&grid), column(&plain), "the line at 0.5");
    assert_eq!(
        pixel(&grid, 700, 650),
        pixel(&plain, 700, 650),
        "between lines"
    );

    let text = f.ok("frame --at 2 --width 640 -o ../малый.png");
    assert!(text.starts_with("кадр 0:02.000 (640×360) → "), "{text}");
    let small = f.png("малый.png");
    assert_eq!((small.size.width, small.size.height), (640, 360));
    assert_eq!(
        pixel(&small, 480, 162),
        [0xef, 0x44, 0x44, 0xff],
        "the shape"
    );

    let dry = f.ok("frame --at 2 -o ../нет.png --dry-run");
    assert!(dry.starts_with("dry-run, файл не записан"), "{dry}");
    assert!(!f.tmp.path().join("нет.png").exists());
}

/// `frame` shows what the export writes at the same moment (same evaluator, sources and
/// compositor): the luma of the PNG and of the MP4 frame differ only by coding noise, while a frame
/// of another moment differs much more.
#[test]
fn frame_is_the_frame_of_the_export() {
    let f = Clicks::new();
    f.ok("annot add shape --at 1 --duration 4 --rect 0.6,0.3,0.3,0.3");
    f.ok("shape set a1 --figure rectangle --fill #ef4444ff --outline false");
    f.ok("zoom add --at 2 --duration 2 --scale 2 --mode fixed --x 0.7 --y 0.4");
    f.ok("cut 5-12");
    f.ok("frame --at 3 -o ../кадр.png");
    f.ok("frame --at 0.5 -o ../другой.png");
    f.ok("export -o ../урок.mp4 --res source --fps 30");
    let video = f.tmp.path().join("урок.mp4");
    let yuv = decoded(&[
        "-i",
        video.to_str().unwrap(),
        "-vf",
        "trim=start_frame=90",
        "-frames:v",
        "1",
        "-pix_fmt",
        "yuv420p",
        "-f",
        "rawvideo",
        "-",
    ]);
    let luma = &yuv[..1280 * 720];
    let same = mean_difference(&f.png("кадр.png"), luma);
    let other = mean_difference(&f.png("другой.png"), luma);
    assert!(same < 1.5, "frame and export differ by {same}");
    assert!(
        other > 10.0 * same,
        "another moment differs by {other} only"
    );
}

/// Mean difference between the BT.709 limited-range luma of `picture` (as the export writes it)
/// and `luma`, 0..255.
pub(crate) fn mean_difference(picture: &Picture, luma: &[u8]) -> f64 {
    let rgba = &picture.frames[0].rgba;
    let total: f64 = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(luma)
        .map(|(&[r, g, b, _], &y)| {
            let [r, g, b] = [r, g, b].map(f64::from);
            let expected = 16.0 + 219.0 * (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0;
            (expected - f64::from(y)).abs()
        })
        .sum();
    total / luma.len() as f64
}

#[test]
fn sheet_puts_frames_of_equal_parts_in_rows() {
    let f = Clicks::new();
    let text = f.ok("sheet --from 1 --to 9 -n 4 -o ../лист.png");
    assert!(
        text.starts_with("раскадровка 4 кадров (2×2): 0:01.000, 0:03.000, 0:05.000, 0:07.000 → "),
        "{text}"
    );
    let sheet = f.png("лист.png");
    let size = |cell: u32| 2 * (cell + SHEET_GAP) + SHEET_GAP;
    assert_eq!(
        (sheet.size.width, sheet.size.height),
        (size(1280), size(720))
    );
    f.ok("sheet --from 1 --to 9 -n 4 --width 664 -o ../малый.png");
    let small = f.png("малый.png");
    assert_eq!((small.size.width, small.size.height), (664, size(180)));
}

#[test]
fn wrong_moments_and_files_are_refused() {
    let f = Clicks::new();
    let registry = dc_ops::Registry::devcast().unwrap();
    let defaults = defaults_file(f.tmp.path());
    for (line, code) in [
        ("frame --at 13 -o ../a.png", 1),
        ("frame --at 2 -o ../a.jpg", 2),
        ("frame --at x -o ../a.png", 2),
        ("sheet --from 5 --to 5 -o ../a.png", 1),
        ("sheet -n 0 -o ../a.png", 2),
        ("sheet -n 37 -o ../a.png", 2),
        ("frame --at 2 --width 1281 -o ../a.png", 1),
        ("frame --at 2 --width 15 -o ../a.png", 1),
        ("sheet -n 4 --width 40 -o ../a.png", 1),
    ] {
        let out = run_line_with(line, &f.root, &registry, &defaults);
        assert_eq!(out.code, code, "{line}: {}", out.stderr);
    }
}
