mod common;

use common::{Planes, TestResult, assert_near, gpu, pixel, render, size, source, video_scene};
use dc_core::{Rect, SourceTime, VideoColor};
use dc_render::{Band, Camera, Compositor, Layer, Nv12, RenderError, Scene, Video};

const BLACK: [u8; 3] = [16, 128, 128];
const WHITE: [u8; 3] = [235, 128, 128];

/// BT.709 limited range: 4×4 blocks of known YUV, drawn 1:1, read inside each block.
#[test]
fn nv12_converts_with_bt709_limited_range() -> TestResult {
    let blocks: [([u8; 3], [u8; 4]); 8] = [
        (BLACK, [0, 0, 0, 255]),
        (WHITE, [255, 255, 255, 255]),
        ([126, 128, 128], [128, 128, 128, 255]),
        ([63, 102, 240], [255, 0, 0, 255]),
        ([173, 42, 26], [0, 255, 0, 255]),
        ([32, 240, 118], [0, 0, 255, 255]),
        ([0, 128, 128], [0, 0, 0, 255]),
        ([255, 128, 128], [255, 255, 255, 255]),
    ];
    let planes = Planes::new(size(16, 8), |x, y| blocks[(y / 4 * 4 + x / 4) as usize].0);
    let image = render(
        &gpu()?,
        &video_scene(size(16, 8), &planes, Rect::FULL),
        &planes,
    )?;
    for (i, (_, expected)) in blocks.iter().enumerate() {
        let (bx, by) = (i as u32 % 4 * 4, i as u32 / 4 * 4);
        for d in [1, 2] {
            assert_near(
                pixel(&image, bx + d, by + d),
                *expected,
                2,
                &format!("block {i}"),
            );
        }
    }
    Ok(())
}

#[test]
fn narrower_clip_gets_black_fields() -> TestResult {
    let planes = Planes::new(size(8, 8), |_, _| WHITE);
    let image = render(
        &gpu()?,
        &video_scene(size(32, 16), &planes, Rect::FULL),
        &planes,
    )?;
    for (x, white) in [
        (0, false),
        (7, false),
        (8, true),
        (16, true),
        (23, true),
        (24, false),
    ] {
        let expected = if white { [255; 4] } else { [0, 0, 0, 255] };
        assert_near(pixel(&image, x, 8), expected, 1, &format!("x {x}"));
    }
    Ok(())
}

#[test]
fn crop_shows_only_its_part() -> TestResult {
    let planes = Planes::new(size(16, 8), |x, _| if x < 8 { BLACK } else { WHITE });
    let crop = Rect::new(0.5, 0.0, 0.5, 1.0)?;
    let image = render(&gpu()?, &video_scene(size(8, 8), &planes, crop), &planes)?;
    for x in 0..8 {
        assert_near(pixel(&image, x, 4), [255; 4], 1, &format!("x {x}"));
    }
    Ok(())
}

#[test]
fn camera_moves_video_but_not_the_overlay_band() -> TestResult {
    let planes = Planes::new(size(16, 16), |_, _| WHITE);
    let mut scene = video_scene(size(16, 16), &planes, Rect::FULL);
    scene.camera = Camera {
        scale: 0.5,
        x: 4.0,
        y: 4.0,
    };
    let image = render(&gpu()?, &scene, &planes)?;
    assert_near(
        pixel(&image, 2, 2),
        [0, 0, 0, 255],
        0,
        "outside the zoomed-out video",
    );
    assert_near(pixel(&image, 8, 8), [255; 4], 1, "inside");
    let video = scene.layers.remove(&Band::Video).unwrap_or_default();
    scene.layers.insert(Band::Overlay, video);
    let image = render(&gpu()?, &scene, &planes)?;
    assert_near(
        pixel(&image, 2, 2),
        [255; 4],
        1,
        "overlay ignores the camera",
    );
    Ok(())
}

#[test]
fn smaller_target_gets_the_same_picture_scaled() -> TestResult {
    let gpu = gpu()?;
    let planes = Planes::new(size(8, 8), |_, _| WHITE);
    let scene = video_scene(size(32, 16), &planes, Rect::FULL);
    let mut compositor = Compositor::new(&gpu);
    compositor.upload(&gpu, &source("media/a.mp4"), planes.nv12()?);
    let target = gpu.target(size(16, 8));
    compositor.draw(&gpu, &scene, &target)?;
    let image = gpu.read(&target)?;
    assert_near(pixel(&image, 1, 4), [0, 0, 0, 255], 0, "field");
    assert_near(pixel(&image, 8, 4), [255; 4], 1, "video");
    Ok(())
}

#[test]
fn odd_sized_frame_is_drawn_whole() -> TestResult {
    let planes = Planes::new(size(5, 3), |_, _| [126, 128, 128]);
    let image = render(
        &gpu()?,
        &video_scene(size(5, 3), &planes, Rect::FULL),
        &planes,
    )?;
    for chunk in image.rgba.as_chunks::<4>().0 {
        assert_near(*chunk, [128, 128, 128, 255], 2, "pixel");
    }
    Ok(())
}

#[test]
fn frame_must_be_uploaded_for_the_shown_time() -> TestResult {
    let gpu = gpu()?;
    let frame = size(8, 8);
    let planes = Planes::new(frame, |_, _| WHITE);
    let mut scene = video_scene(frame, &planes, Rect::FULL);
    let target = gpu.target(frame);
    let mut compositor = Compositor::new(&gpu);
    let missing = compositor.draw(&gpu, &scene, &target);
    assert!(
        matches!(missing, Err(RenderError::NoFrame { .. })),
        "{missing:?}"
    );
    compositor.upload(&gpu, &source("media/a.mp4"), planes.nv12()?);
    compositor.draw(&gpu, &scene, &target)?;
    let Some(Layer::Video(video)) = scene
        .layers
        .get_mut(&Band::Video)
        .and_then(|l| l.first_mut())
    else {
        panic!("no video layer");
    };
    video.source.time = SourceTime::from_micros(1_000_000);
    let stale = compositor.draw(&gpu, &scene, &target);
    assert!(
        matches!(stale, Err(RenderError::NoFrame { .. })),
        "{stale:?}"
    );
    Ok(())
}

#[test]
fn frames_the_scene_does_not_show_are_dropped() -> TestResult {
    let gpu = gpu()?;
    let frame = size(8, 8);
    let planes = Planes::new(frame, |_, _| WHITE);
    let scene_of = |media: &str| {
        let mut scene = Scene::new(frame);
        let video = Video::fitted(source(media), frame, Rect::FULL, frame);
        scene.push(Band::Video, Layer::Video(video));
        scene
    };
    let target = gpu.target(frame);
    let mut compositor = Compositor::new(&gpu);
    compositor.upload(&gpu, &source("media/a.mp4"), planes.nv12()?);
    compositor.upload(&gpu, &source("media/b.mp4"), planes.nv12()?);
    compositor.draw(&gpu, &scene_of("media/a.mp4"), &target)?;
    let dropped = compositor.draw(&gpu, &scene_of("media/b.mp4"), &target);
    assert!(
        matches!(dropped, Err(RenderError::NoFrame { .. })),
        "{dropped:?}"
    );
    Ok(())
}

#[test]
fn nv12_planes_must_match_the_size() {
    let y = [0; 15];
    let uv = [0; 12];
    assert!(Nv12::new(size(5, 3), &y, &uv, VideoColor::BT709).is_ok());
    let short = Nv12::new(size(5, 3), &y, &uv[..10], VideoColor::BT709);
    assert!(matches!(short, Err(RenderError::Nv12 { .. })), "{short:?}");
    assert!(Nv12::new(size(0, 0), &[], &[], VideoColor::BT709).is_err());
}

/// H.264 sites chroma on even luma columns: luma column 2 takes chroma sample 1 alone.
#[test]
fn chroma_is_sited_left() -> TestResult {
    let planes = Planes::new(size(8, 2), |x, _| {
        if x < 2 {
            [126, 128, 240]
        } else {
            [126, 128, 128]
        }
    });
    let image = render(
        &gpu()?,
        &video_scene(size(8, 2), &planes, Rect::FULL),
        &planes,
    )?;
    assert_near(pixel(&image, 2, 0), [128, 128, 128, 255], 2, "column 2");
    let half = pixel(&image, 1, 0);
    assert!(
        half[0] > 160 && half[0] < 230,
        "column 1 mixes samples 0 and 1: {half:?}"
    );
    Ok(())
}
