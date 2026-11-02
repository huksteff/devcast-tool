use std::path::PathBuf;

use dc_core::project::MediaKind;
use dc_core::{SourceTime, TimeDelta};

use super::*;

fn fixture() -> (PathBuf, Media) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings/clicks.devcast/media/recording-1");
    let media = Media {
        path: "media/recording-1/video.mp4".to_owned(),
        kind: MediaKind::Recording,
        duration: TimeDelta::from_secs_f64(12.5),
        size: PixelSize {
            width: 1280,
            height: 720,
        },
    };
    (dir, media)
}

#[test]
fn clicks_of_the_fixture_land_on_their_points() {
    let (dir, media) = fixture();
    let track = Telemetry::read(&dir)
        .unwrap()
        .cursor_track(&dir, &media)
        .unwrap();
    // 6 left clicks, then a double and a right click at (400, 400) (fixtures/recordings/README.md).
    let expected = [
        (200, 150),
        (1080, 150),
        (640, 360),
        (200, 600),
        (1080, 600),
        (900, 300),
        (400, 400),
        (400, 400),
        (400, 400),
    ];
    assert_eq!(track.clicks.len(), expected.len());
    for (click, (x, y)) in track.clicks.iter().zip(expected) {
        assert!(
            (click.pos.x * 1280.0 - f64::from(x)).abs() <= 2.0,
            "{click:?}"
        );
        assert!(
            (click.pos.y * 720.0 - f64::from(y)).abs() <= 2.0,
            "{click:?}"
        );
        assert!(track.moves.iter().any(|m| m == click));
    }
    assert!(track.moves.windows(2).all(|w| w[0].at <= w[1].at));
    assert!(track.moves.len() > 100);
}

#[test]
fn shapes_point_at_the_pictures_next_to_the_video() {
    let (dir, media) = fixture();
    let track = Telemetry::read(&dir)
        .unwrap()
        .cursor_track(&dir, &media)
        .unwrap();
    assert!(track.shapes.first().unwrap().at <= SourceTime::from_secs_f64(0.1));
    let pictures: Vec<_> = track
        .shapes
        .iter()
        .filter_map(|s| s.picture.as_ref())
        .collect();
    assert!(pictures.len() >= 2, "{:?}", track.shapes);
    for picture in pictures {
        assert!(picture.path.starts_with("media/recording-1/cursor-"));
        assert!(picture.size.width >= 16 && picture.size.height >= 16);
        assert!(picture.hotspot.x < f64::from(picture.size.width));
        assert!(picture.hotspot.y < f64::from(picture.size.height));
    }
}

#[test]
fn a_file_that_is_not_png_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cursor-0000.png");
    std::fs::write(&path, b"not a picture at all, just text").unwrap();
    let error = png_size(&path).unwrap_err();
    assert!(error.to_string().contains("PNG"), "{error}");
    assert!(png_size(&dir.path().join("missing.png")).is_err());
}

#[test]
fn key_presses_of_the_fixture_read_like_on_the_keyboard() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings/typing.devcast/media/recording-1");
    let presses = Telemetry::read(&dir).unwrap().key_presses();
    let names: Vec<&str> = presses.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Enter",
            "Ctrl+A",
            "Ctrl+C",
            "Ctrl+V",
            "Ctrl+Z",
            "Ctrl+Shift+P",
            "Alt+Shift+F",
            "F5",
            "Left",
            "Left",
            "Left",
            "Delete",
            "Esc",
        ]
    );
    assert!(presses.windows(2).all(|w| w[0].at <= w[1].at));
    assert!(presses[0].at > SourceTime::ZERO);
}
