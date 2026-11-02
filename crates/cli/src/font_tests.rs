//! `devcast font add` and `font ls` on a copy of the fixture `clicks.devcast`, and a text
//! annotation in the added font: `frame` and the export draw it, a family not added is refused
//! with a hint. The font is `fixtures/fonts/PressStart2P-Regular.ttf` (OFL, family
//! «Press Start 2P»).

use std::path::{Path, PathBuf};

use dc_media::{Picture, decode_picture};

use crate::run::{Local, Outcome, run_with};
use crate::test_util::{decoded, defaults_file, fixture_copy};

const FAMILY: &str = "Press Start 2P";

struct Lesson {
    tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Lesson {
    /// The fixture with the font file `шрифты/пиксели.ttf` next to the project file.
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Урок со шрифтом.devcast");
        fixture_copy("clicks.devcast", &root);
        let fonts = root.join("шрифты");
        std::fs::create_dir(&fonts).unwrap();
        std::fs::copy(fixture_font(), fonts.join("пиксели.ttf")).unwrap();
        Self { tmp, root }
    }

    fn run(&self, line: &str) -> Outcome {
        self.run_args(line.split(' ').map(str::to_owned).collect())
    }

    fn run_args(&self, args: Vec<String>) -> Outcome {
        let defaults = defaults_file(self.tmp.path());
        let channel = format!("test-export-font-{}", std::process::id());
        let local = Local {
            defaults: Some(&defaults),
            export_channel: Some(&channel),
            ..Local::default()
        };
        run_with(
            args,
            &self.root,
            &dc_ops::Registry::devcast().unwrap(),
            local,
        )
    }

    fn ok(&self, line: &str) -> String {
        let out = self.run(line);
        assert_eq!(out.code, 0, "{line}: {}", out.stderr);
        out.stdout.trim_end().to_owned()
    }

    fn png(&self, name: &str) -> Picture {
        decode_picture(&self.tmp.path().join(name)).unwrap()
    }
}

fn fixture_font() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts/PressStart2P-Regular.ttf")
}

#[test]
fn font_add_copies_the_file_once_and_ls_lists_it() {
    let f = Lesson::new();
    let dry = f.ok("font add шрифты/пиксели.ttf --dry-run");
    assert!(dry.starts_with("dry-run"), "{dry}");
    assert!(!f.root.join("assets/fonts").exists());
    assert_eq!(
        f.ok("font add шрифты/пиксели.ttf"),
        format!("шрифт «{FAMILY}»: assets/fonts/пиксели.ttf")
    );
    assert!(f.root.join("assets/fonts/пиксели.ttf").is_file());
    let again = f.ok("font add шрифты/пиксели.ttf");
    assert!(again.ends_with("(уже был в проекте)"), "{again}");
    let listed = f.ok("font ls");
    assert!(listed.starts_with("Inter  встроенный\n"), "{listed}");
    assert!(
        listed.ends_with(&format!("{FAMILY}  assets/fonts/пиксели.ttf")),
        "{listed}"
    );
}

#[test]
fn files_that_are_not_new_fonts_are_refused() {
    let f = Lesson::new();
    std::fs::write(f.root.join("шрифты/пусто.ttf"), b"not a font").unwrap();
    let out = f.run("font add шрифты/пусто.ttf");
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("нет шрифта"), "{}", out.stderr);
    let out = f.run("font add project.json");
    assert!(out.stderr.contains("не файл шрифта"), "{}", out.stderr);
    let out = f.run("font add шрифты/нет.ttf");
    assert!(out.stderr.contains("не найден"), "{}", out.stderr);
    let inter =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.deps/fonts/Roboto[wdth,wght].ttf");
    std::fs::copy(inter, f.root.join("шрифты/свой.ttf")).unwrap();
    let out = f.run("font add шрифты/свой.ttf");
    assert!(out.stderr.contains("уже встроен"), "{}", out.stderr);
    assert!(!f.root.join("assets/fonts").exists());
}

/// Black words over the white screen of the fixture, at output 1–4 s; `cut 5-12` keeps the
/// export short.
fn words(f: &Lesson) {
    f.ok("cut 5-12");
    f.ok("annot add text --at 1 --duration 3 --rect 0.1,0.3,0.8,0.4");
    f.ok("text set a1 --text DEVCAST --size 128 --color #000000ff --background #00000000");
}

#[test]
fn frame_and_export_draw_the_added_font() {
    let f = Lesson::new();
    words(&f);
    f.ok("frame --at 2 -o ../inter.png");
    let font = ["text", "set", "a1", "--font", FAMILY].map(str::to_owned);
    assert_eq!(f.run_args(font.to_vec()).code, 0);
    let out = f.run("frame --at 2 -o ../нет.png");
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("devcast font add"), "{}", out.stderr);
    f.ok("font add шрифты/пиксели.ttf");
    f.ok("frame --at 2 -o ../пиксели.png");
    let (inter, own) = (f.png("inter.png"), f.png("пиксели.png"));
    let differ = inter.frames[0]
        .rgba
        .iter()
        .zip(&own.frames[0].rgba)
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        differ > 10_000,
        "the words look the same: {differ} bytes differ"
    );
    f.ok("export -o ../урок.mp4 --res source --fps 30");
    let video = f.tmp.path().join("урок.mp4");
    let yuv = decoded(&[
        "-i",
        video.to_str().unwrap(),
        "-vf",
        "trim=start_frame=60",
        "-frames:v",
        "1",
        "-pix_fmt",
        "yuv420p",
        "-f",
        "rawvideo",
        "-",
    ]);
    let luma = &yuv[..1280 * 720];
    let same = crate::still_tests::mean_difference(&own, luma);
    let other = crate::still_tests::mean_difference(&inter, luma);
    assert!(
        same < other,
        "the export is closer to Inter: {same} vs {other}"
    );
}
