//! Reference frames of the project's scene (decision 012) for task 5.16: the output frame of
//! every aspect, a crop and clips of other proportions fitted with black fields. Rendered on the
//! software adapter like CI; a frame that changes on purpose is updated with
//! `cargo xtask golden update "<почему>"`.

use std::error::Error;

use dc_core::project::{Media, MediaKind};
use dc_core::{
    Clip, OutputTime, PixelSize, Project, Range, Rect, SourceTime, TimeDelta, VideoColor,
};
use dc_fx_crop::{Aspect, Crop, CropSettings, CropState};
use dc_ops::Evaluator;
use dc_render::golden::Golden;
use dc_render::{AdapterChoice, Compositor, Gpu, Layer, Nv12, Scene};

type TestResult = Result<(), Box<dyn Error>>;

/// Colour bars over a luma ramp, BT.709 limited: every edge of the picture is visible.
fn bars(size: PixelSize) -> (Vec<u8>, Vec<u8>) {
    const YUV: [[u8; 3]; 6] = [
        [235, 128, 128],
        [63, 102, 240],
        [173, 42, 26],
        [32, 240, 118],
        [126, 128, 128],
        [81, 90, 240],
    ];
    let (w, h) = (size.width, size.height);
    let yuv = |x: u32, y: u32| {
        if y * 3 < h * 2 {
            YUV[(x * 6 / w) as usize]
        } else {
            [(16 + x * 219 / (w - 1)) as u8, 128, 128]
        }
    };
    let luma = (0..h).flat_map(|r| (0..w).map(move |c| yuv(c, r)[0]));
    let chroma = (0..h / 2)
        .flat_map(|r| (0..w / 2).map(move |c| (r, c)))
        .flat_map(|(r, c)| {
            let [_, u, v] = yuv(c * 2, r * 2);
            [u, v]
        });
    (luma.collect(), chroma.collect())
}

fn clip(project: &mut Project, path: &str, size: PixelSize, crop: Rect) -> TestResult {
    let id = project.clip_ids.allocate();
    let duration = TimeDelta::from_secs_f64(2.0);
    project.clips.push(Clip {
        id,
        media: Media {
            path: path.to_owned(),
            kind: MediaKind::Recording,
            duration,
            size,
        },
        trim: Range::new(SourceTime::ZERO, SourceTime::ZERO + duration)?,
        cuts: Vec::new(),
        speeds: Vec::new(),
        crop,
    });
    Ok(())
}

fn size(width: u32, height: u32) -> PixelSize {
    PixelSize { width, height }
}

fn with_aspect(mut project: Project, aspect: Aspect) -> Result<Project, Box<dyn Error>> {
    let state = CropState {
        settings: CropSettings { aspect },
    };
    project.set_state::<Crop>(&state)?;
    Ok(project)
}

/// Renders the project's scene at output second `at` with bars as every video and checks it
/// against reference `crop/<name>`.
fn check(name: &str, project: &Project, at: f64) -> TestResult {
    let scene: Scene = Evaluator::new(project)?.evaluate(OutputTime::from_secs_f64(at));
    let gpu = Gpu::new(AdapterChoice::Software)?;
    let mut compositor = Compositor::new(&gpu);
    for (_, layer) in scene.painted() {
        if let Layer::Video(video) = layer {
            let media = project
                .clips
                .iter()
                .find(|c| c.media.path == video.source.media)
                .ok_or("no clip of the video layer")?;
            let (y, uv) = bars(media.media.size);
            let nv12 = Nv12::new(media.media.size, &y, &uv, VideoColor::BT709)?;
            compositor.upload(&gpu, &video.source, nv12);
        }
    }
    let image = compositor.render(&gpu, &scene)?;
    Golden::new(&gpu)?
        .check(&format!("crop/{name}"), &image)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn golden_4x3_clip_in_a_16x9_frame() -> TestResult {
    let mut project = Project::default();
    clip(&mut project, "media/a.mp4", size(160, 120), Rect::FULL)?;
    check("aspect_16x9", &with_aspect(project, Aspect::Wide16x9)?, 0.5)
}

#[test]
fn golden_free_frame_is_the_crop() -> TestResult {
    let mut project = Project::default();
    let crop = Rect::new(0.25, 0.1, 0.5, 0.6)?;
    clip(&mut project, "media/a.mp4", size(320, 180), crop)?;
    check("free", &with_aspect(project, Aspect::Free)?, 0.5)
}

#[test]
fn golden_second_clip_of_other_proportions_gets_fields() -> TestResult {
    let mut project = Project::default();
    clip(&mut project, "media/a.mp4", size(320, 180), Rect::FULL)?;
    let top_half = Rect::new(0.0, 0.0, 1.0, 0.5)?;
    clip(&mut project, "media/b.mp4", size(160, 120), top_half)?;
    check("mixed_clips", &with_aspect(project, Aspect::Source)?, 2.5)
}
