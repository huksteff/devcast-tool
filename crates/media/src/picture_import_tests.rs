use std::path::PathBuf;

use dc_core::PixelSize;
use dc_project::ProjectDir;

use super::*;
use crate::test_util::ffmpeg;

struct Setup {
    _tmp: tempfile::TempDir,
    project: ProjectDir,
    pictures: PathBuf,
}

fn setup() -> Setup {
    let tmp = tempfile::tempdir().unwrap();
    let project = ProjectDir::create(tmp.path().join("Урок.dcproj")).unwrap();
    let pictures = tmp.path().join("Мои картинки");
    std::fs::create_dir(&pictures).unwrap();
    Setup {
        _tmp: tmp,
        project,
        pictures,
    }
}

const FOLDER: &str = "assets/images";

#[test]
fn a_picture_is_read_and_copied_and_the_same_file_is_not_copied_twice() {
    let s = setup();
    let logo = s.pictures.join("лого 1.PNG");
    ffmpeg(
        "-f lavfi -i color=c=red:s=6x4 -frames:v 1 -f image2 -c:v png",
        &logo,
    );
    let first = import_picture(&s.project, FOLDER, &logo, false).unwrap();
    assert_eq!(first.path, "assets/images/лого 1.png");
    assert!(first.copied);
    let size = PixelSize {
        width: 6,
        height: 4,
    };
    assert_eq!(first.picture.size, size);
    let copy = s.project.assets_dir().join("images").join("лого 1.png");
    assert_eq!(std::fs::read(&copy).unwrap(), std::fs::read(&logo).unwrap());
    let again = import_picture(&s.project, FOLDER, &logo, false).unwrap();
    assert_eq!(
        (again.path.as_str(), again.copied),
        (first.path.as_str(), false)
    );
}

#[test]
fn another_file_of_the_same_name_gets_a_number_and_a_dry_run_copies_nothing() {
    let s = setup();
    let a = s.pictures.join("a.png");
    ffmpeg(
        "-f lavfi -i color=c=red:s=6x4 -frames:v 1 -f image2 -c:v png",
        &a,
    );
    import_picture(&s.project, FOLDER, &a, false).unwrap();
    let other = s.pictures.join("другая");
    std::fs::create_dir(&other).unwrap();
    let b = other.join("a.png");
    ffmpeg(
        "-f lavfi -i color=c=blue:s=3x3 -frames:v 1 -f image2 -c:v png",
        &b,
    );
    let dry = import_picture(&s.project, FOLDER, &b, true).unwrap();
    assert_eq!(
        (dry.path.as_str(), dry.copied),
        ("assets/images/a-2.png", false)
    );
    assert!(!s.project.assets_dir().join("images/a-2.png").exists());
    let real = import_picture(&s.project, FOLDER, &b, false).unwrap();
    assert_eq!(
        (real.path.as_str(), real.copied),
        ("assets/images/a-2.png", true)
    );
}

#[test]
fn an_animated_gif_keeps_its_frames() {
    let s = setup();
    let gif = s.pictures.join("blink.gif");
    ffmpeg("-f lavfi -i testsrc2=s=8x8:r=5:d=0.6", &gif);
    let imported = import_picture(&s.project, FOLDER, &gif, false).unwrap();
    assert_eq!(imported.picture.frames.len(), 3);
}

#[test]
fn svg_other_files_and_missing_files_are_refused_before_copying() {
    let s = setup();
    let svg = s.pictures.join("icon.svg");
    std::fs::write(&svg, "<svg xmlns='http://www.w3.org/2000/svg'/>").unwrap();
    let error = import_picture(&s.project, FOLDER, &svg, false).unwrap_err();
    assert!(matches!(error, MediaError::Svg { .. }), "{error}");
    assert!(error.to_string().contains("PNG"), "{error}");
    let text = s.pictures.join("notes.txt");
    std::fs::write(&text, "x").unwrap();
    let error = import_picture(&s.project, FOLDER, &text, false).unwrap_err();
    assert!(matches!(error, MediaError::NotAPicture { .. }), "{error}");
    let missing = s.pictures.join("нет.png");
    let error = import_picture(&s.project, FOLDER, &missing, false).unwrap_err();
    assert!(matches!(error, MediaError::NotFound { .. }), "{error}");
    let broken = s.pictures.join("битая.png");
    std::fs::write(&broken, "not a png").unwrap();
    let error = import_picture(&s.project, FOLDER, &broken, false).unwrap_err();
    assert!(matches!(error, MediaError::Picture { .. }), "{error}");
    assert!(!s.project.assets_dir().join("images").exists());
}
