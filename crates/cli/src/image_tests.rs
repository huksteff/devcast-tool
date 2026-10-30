use dc_fx_annotations::Annotations;
use dc_fx_image::ImageBody;

use crate::test_util::{Recordings, ffmpeg};

/// The image annotation a1 at output 1–3 s of `Recordings`, with `картинки/лого.png` (6×4) and
/// `картинки/мигание.gif` (3 frames of 200 ms) next to the project file.
fn project() -> Recordings {
    let r = Recordings::new();
    let folder = r.dir.root().join("картинки");
    std::fs::create_dir(&folder).unwrap();
    let png = folder.join("лого.png");
    let gif = folder.join("мигание.gif");
    let png_args = ["-f", "lavfi", "-i", "color=c=red:s=6x4", "-frames:v", "1"];
    ffmpeg(&[&png_args[..], &[png.to_str().unwrap()]].concat());
    let gif_args = ["-f", "lavfi", "-i", "testsrc2=s=8x8:r=5:d=0.6"];
    ffmpeg(&[&gif_args[..], &[gif.to_str().unwrap()]].concat());
    r.ok("annot add image --at 1 --duration 2");
    r
}

fn body(r: &Recordings) -> ImageBody {
    let project = r.dir.load().unwrap();
    project.state::<Annotations>().unwrap().annotations[0]
        .body()
        .unwrap()
}

#[test]
fn image_set_copies_the_picture_and_undo_takes_it_back() {
    let r = project();
    assert_eq!(
        r.ok("image set a1 --file картинки/лого.png"),
        "аннотации ~1 a1"
    );
    let picture = body(&r).picture.unwrap();
    assert_eq!(picture.path, "assets/images/лого.png");
    assert_eq!((picture.size, picture.frames.is_empty()), ([6, 4], true));
    assert!(r.dir.root().join("assets/images/лого.png").is_file());
    r.ok("undo");
    assert_eq!(body(&r).picture, None);
    // The same file again is not copied twice.
    r.ok("image set a1 --file картинки/лого.png");
    let names: Vec<_> = std::fs::read_dir(r.dir.root().join("assets/images"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
}

#[test]
fn an_animated_gif_keeps_its_frame_timing_and_a_moment_makes_a_key() {
    let r = project();
    r.ok("image set a1 --file картинки/лого.png");
    r.ok("image set a1 --file картинки/мигание.gif --at 2");
    let project = r.dir.load().unwrap();
    let a = &project.state::<Annotations>().unwrap().annotations[0];
    assert_eq!(a.keys.len(), 2, "a key at the start and at 2 s");
    let key: ImageBody = serde_json::from_value(a.keys[1].body.clone().into()).unwrap();
    let gif = key.picture.unwrap();
    assert_eq!(gif.path, "assets/images/мигание.gif");
    assert_eq!(gif.frames, [[3, 200]]);
}

#[test]
fn a_failed_change_leaves_no_copy_and_bad_files_are_refused() {
    let r = project();
    r.ok("annot add text --at 5");
    let out = r.run("image set a2 --file картинки/лого.png");
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stderr.contains("не картинка"), "{}", out.stderr);
    assert!(!r.dir.root().join("assets/images/лого.png").exists());
    std::fs::write(r.dir.root().join("картинки/знак.svg"), "<svg/>").unwrap();
    let out = r.run("image set a1 --file картинки/знак.svg");
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("SVG"), "{}", out.stderr);
    let out = r.run("image set a1 --file картинки/нет.png");
    assert!(out.stderr.contains("не найден"), "{}", out.stderr);
    let out = r.run("image set x1 --file картинки/лого.png");
    assert_eq!(out.code, 2, "{}", out.stderr);
}

#[test]
fn a_dry_run_copies_nothing() {
    let r = project();
    let out = r.ok("image set a1 --file картинки/лого.png --dry-run");
    assert!(out.starts_with("dry-run"), "{out}");
    assert!(!r.dir.root().join("assets/images").exists());
    assert_eq!(body(&r).picture, None);
}
