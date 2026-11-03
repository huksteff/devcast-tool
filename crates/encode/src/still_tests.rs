use std::path::Path;
use std::process::Command;

use dc_core::{PixelSize, Rect, SourceTime};
use dc_media::encode_png;
use dc_render::{AdapterChoice, ImageKey, PxRect, Sprite, Video, VideoSource};

use super::*;

const SIZE: PixelSize = PixelSize {
    width: 64,
    height: 36,
};

/// A second of grey, 64×36 at 10 fps.
fn grey(path: &Path) {
    let exe = Path::new(env!("FFMPEG_DIR"))
        .join("bin")
        .join(format!("ffmpeg{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new(exe)
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=gray:s=64x36:r=10:d=1",
        ])
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Two videos and a picture.
fn project(dir: &Path) -> ProjectDir {
    let project = ProjectDir::create(dir.join("p.devcast")).unwrap();
    grey(&project.media_dir().join("a.mp4"));
    grey(&project.media_dir().join("b.mp4"));
    let png = encode_png(SIZE, &[0, 255, 0, 255].repeat(64 * 36)).unwrap();
    std::fs::write(project.assets_dir().join("dot.png"), png).unwrap();
    project
}

/// Video `media` at 0.5 s, with the picture over it when `picture`.
fn scene(media: &str, picture: bool) -> Scene {
    let mut scene = Scene::new(SIZE);
    let source = VideoSource {
        media: format!("media/{media}"),
        time: SourceTime::from_micros(500_000),
    };
    let video = Video::fitted(source, SIZE, Rect::FULL, SIZE);
    scene.push(dc_render::Band::Video, Layer::Video(video));
    if picture {
        let sprite = Sprite {
            image: ImageKey {
                path: "assets/dot.png".into(),
                frame: 0,
            },
            rect: PxRect {
                x: 0.0,
                y: 0.0,
                w: 8.0,
                h: 8.0,
            },
            rotation: 0.0,
            opacity: 1.0,
            motion: None,
        };
        scene.push(dc_render::Band::Annotations, Layer::Sprite(sprite));
    }
    scene
}

fn kept(sources: &Sources<'_>) -> (Vec<String>, usize) {
    let mut decoders: Vec<String> = sources.decoders.keys().cloned().collect();
    decoders.sort();
    (decoders, sources.pictures.len())
}

#[test]
fn decoders_and_pictures_no_scene_uses_are_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let project = project(dir.path());
    let gpu = Gpu::new(AdapterChoice::Software).unwrap();
    let mut compositor = Compositor::new(&gpu);
    let mut sources = Sources::new(&project);
    sources
        .feed(&gpu, &mut compositor, &scene("a.mp4", true))
        .unwrap();
    let both = vec!["media/a.mp4".to_owned()];
    assert_eq!(kept(&sources), (both, 1));
    for _ in 1..UNUSED_FEEDS {
        sources
            .feed(&gpu, &mut compositor, &scene("b.mp4", false))
            .unwrap();
    }
    let open = vec!["media/a.mp4".to_owned(), "media/b.mp4".to_owned()];
    assert_eq!(kept(&sources), (open, 1), "not yet");
    sources
        .feed(&gpu, &mut compositor, &scene("b.mp4", false))
        .unwrap();
    assert_eq!(kept(&sources), (vec!["media/b.mp4".to_owned()], 0));
}
