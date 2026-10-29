use dc_core::{PixelSize, TimeDelta};

use super::*;
use crate::test_util::ffmpeg;

fn pixel(picture: &Picture, frame: usize, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * picture.size.width + x) * 4) as usize;
    let rgba = &picture.frames[frame].rgba;
    [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
}

fn near(actual: [u8; 4], expected: [u8; 4], tolerance: u8) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(a, e)| a.abs_diff(e) <= tolerance)
}

#[test]
fn png_keeps_straight_alpha() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("полупрозрачная картинка.png");
    ffmpeg(
        "-f lavfi -i color=c=red@0.5:s=8x4,format=rgba -frames:v 1",
        &path,
    );
    let picture = decode_picture(&path).unwrap();
    assert_eq!(
        picture.size,
        PixelSize {
            width: 8,
            height: 4
        }
    );
    assert_eq!(picture.frames.len(), 1);
    assert_eq!(picture.frames[0].rgba.len(), 8 * 4 * 4);
    assert_eq!(picture.frames[0].duration, TimeDelta::ZERO);
    let p = pixel(&picture, 0, 3, 2);
    assert!(near(p, [255, 0, 0, 128], 1), "{p:?}");
}

#[test]
fn jpeg_and_webp_decode_to_rgba() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["a.jpg", "a.webp"] {
        let path = dir.path().join(name);
        ffmpeg(
            "-f lavfi -i color=c=0x2060c0:s=17x9 -frames:v 1 -q:v 2",
            &path,
        );
        let picture = decode_picture(&path).unwrap();
        assert_eq!(
            picture.size,
            PixelSize {
                width: 17,
                height: 9
            },
            "{name}"
        );
        let p = pixel(&picture, 0, 8, 4);
        assert!(near(p, [0x20, 0x60, 0xc0, 255], 6), "{name}: {p:?}");
    }
}

/// Three 100 ms frames: red, green, blue.
const ANIMATION: &str = r"-f lavfi -i color=s=8x8:r=10,format=gbrp,geq=r=255*eq(N\,0):g=255*eq(N\,1):b=255*eq(N\,2) -frames:v 3";

#[test]
fn animated_gif_and_webp_give_every_frame_with_its_time() {
    let dir = tempfile::tempdir().unwrap();
    for (name, codec) in [("a.gif", "gif"), ("a.webp", "libwebp_anim -lossless 1")] {
        let path = dir.path().join(name);
        ffmpeg(&format!("{ANIMATION} -c:v {codec} -loop 0"), &path);
        let picture = decode_picture(&path).unwrap();
        assert_eq!(picture.frames.len(), 3, "{name}");
        let colours = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
        for (i, colour) in colours.into_iter().enumerate() {
            let p = pixel(&picture, i, 4, 4);
            assert!(near(p, colour, 8), "{name} frame {i}: {p:?}");
            assert_eq!(
                picture.frames[i].duration,
                TimeDelta::from_micros(100_000),
                "{name} frame {i}"
            );
        }
    }
}

#[test]
fn not_a_picture_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.png");
    std::fs::write(&path, "не картинка").unwrap();
    let error = decode_picture(&path).unwrap_err();
    assert!(matches!(error, MediaError::Picture { .. }), "{error}");
    assert!(error.to_string().contains("PNG"), "{error}");
}

#[test]
fn png_round_trips_rgba_exactly() {
    let size = PixelSize {
        width: 5,
        height: 3,
    };
    let rgba: Vec<u8> = (0..5 * 3 * 4).map(|i| (i * 17 % 256) as u8).collect();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("кадр.png");
    std::fs::write(&path, encode_png(size, &rgba).unwrap()).unwrap();
    let picture = decode_picture(&path).unwrap();
    assert_eq!(picture.size, size);
    assert_eq!(picture.frames[0].rgba, rgba);
    assert!(matches!(
        encode_png(size, &rgba[4..]),
        Err(MediaError::EncodePng(_))
    ));
}
