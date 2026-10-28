use dc_core::PixelSize;
use dc_project::ProjectDir;

use super::*;
use crate::test_util::{ffmpeg_from, testsrc};

fn size(width: u32, height: u32) -> PixelSize {
    PixelSize { width, height }
}

#[test]
fn fit_keeps_the_aspect_and_never_scales_up() {
    assert_eq!(fit(size(1920, 1080)), size(320, 180));
    assert_eq!(fit(size(1280, 720)), size(320, 180));
    assert_eq!(fit(size(1080, 1920)), size(101, 180));
    assert_eq!(fit(size(3440, 1440)), size(320, 134));
    assert_eq!(fit(size(160, 120)), size(160, 120));
    assert_eq!(fit(size(10_000, 2)), size(320, 1));
}

/// A project with `media/запись 1.mp4` (1280×720, 1 s).
fn project() -> (tempfile::TempDir, ProjectDir) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = ProjectDir::create(tmp.path().join("Урок.dcproj")).unwrap();
    crate::test_util::ffmpeg(
        "-f lavfi -i testsrc2=size=1280x720:rate=30 -t 1 -c:v libx264 -g 30 -pix_fmt yuv420p",
        &dir.media_dir().join("запись 1.mp4"),
    );
    (tmp, dir)
}

#[test]
fn preview_is_the_first_frame_scaled_like_ffmpeg_does() {
    let (_tmp, dir) = project();
    let path = preview(&dir, "media/запись 1.mp4").unwrap();
    assert!(
        path.starts_with(dir.cache_dir().join("media")),
        "{}",
        path.display()
    );
    let ours = path.with_extension("rgb");
    ffmpeg_from(&path, "-f rawvideo -pix_fmt rgb24", &ours);
    let theirs = path.with_extension("ffmpeg.rgb");
    ffmpeg_from(
        &dir.media_dir().join("запись 1.mp4"),
        "-frames:v 1 -vf scale=320:180:flags=area:in_color_matrix=bt709:in_range=tv:out_range=pc -f rawvideo -pix_fmt rgb24",
        &theirs,
    );
    let (ours, theirs) = (std::fs::read(ours).unwrap(), std::fs::read(theirs).unwrap());
    assert_eq!(ours.len(), 320 * 180 * 3);
    let worst = ours.iter().zip(&theirs).map(|(a, b)| a.abs_diff(*b)).max();
    assert!(worst <= Some(2), "{worst:?}");
}

#[test]
fn a_changed_video_gets_a_new_preview() {
    let (_tmp, dir) = project();
    let first = preview(&dir, "media/запись 1.mp4").unwrap();
    assert_eq!(preview(&dir, "media/запись 1.mp4").unwrap(), first);
    testsrc(
        &dir.media_dir().join("запись 1.mp4"),
        "libx264",
        30,
        30,
        2.0,
    );
    let second = preview(&dir, "media/запись 1.mp4").unwrap();
    assert_ne!(second, first);
    assert!(!first.exists());
    let folder = std::fs::read_dir(second.parent().unwrap()).unwrap();
    let previews = folder
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "png")
        })
        .count();
    assert_eq!(previews, 1);
}

#[test]
fn project_preview_is_of_the_first_clip() {
    let (_tmp, dir) = project();
    let media = crate::import(&dir, &[dir.media_dir().join("запись 1.mp4")]).unwrap();
    let mut project = dc_core::Project::default();
    assert_eq!(project_preview(&dir, &project).unwrap(), None);
    for (i, m) in media.into_iter().enumerate() {
        project.clips.push(dc_core::Clip {
            id: dc_core::ClipId::from_number(i as u32 + 1),
            trim: dc_core::Range::new(
                dc_core::SourceTime::ZERO,
                dc_core::SourceTime::ZERO + m.duration,
            )
            .unwrap(),
            media: m,
            cuts: Vec::new(),
            speeds: Vec::new(),
            crop: dc_core::Rect::FULL,
        });
    }
    let path = project_preview(&dir, &project).unwrap().unwrap();
    assert_eq!(path, preview(&dir, "media/запись 1-2.mp4").unwrap());
}
