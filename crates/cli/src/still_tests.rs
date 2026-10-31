//! `devcast frame` and `devcast sheet` on a copy of the fixture `clicks.dcproj` (a white screen
//! with a cursor) with annotations: what the evaluator shows reaches the PNG, the grid lies on
//! top, the sheet has its frames in rows. Real GPU or WARP, like the export tests.

use std::path::{Path, PathBuf};

use dc_media::{Picture, decode_picture};
use dc_render::SHEET_GAP;

use crate::test_util::{defaults_file, fixture_copy, run_line_with};

struct Clicks {
    tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Clicks {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Кадры урока.dcproj");
        fixture_copy("clicks.dcproj", &root);
        let sticker =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/images/sticker.png");
        std::fs::copy(sticker, root.join("наклейка.png")).unwrap();
        Self { tmp, root }
    }

    fn ok(&self, line: &str) -> String {
        let defaults = defaults_file(self.tmp.path());
        let out = run_line_with(
            line,
            &self.root,
            &dc_ops::Registry::devcast().unwrap(),
            &defaults,
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

    let dry = f.ok("frame --at 2 -o ../нет.png --dry-run");
    assert!(dry.starts_with("dry-run, файл не записан"), "{dry}");
    assert!(!f.tmp.path().join("нет.png").exists());
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
    ] {
        let out = run_line_with(line, &f.root, &registry, &defaults);
        assert_eq!(out.code, code, "{line}: {}", out.stderr);
    }
}
