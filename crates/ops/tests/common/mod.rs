//! Shared by the golden tests: projects of bar videos rendered on the software adapter like CI.
//! Cursor pictures come from the fixture recording `clicks.dcproj`, pictures of image annotations
//! from `fixtures/images`.

use std::error::Error;
use std::path::{Path, PathBuf};

use dc_core::project::{Media, MediaKind};
use dc_core::{
    Clip, CursorTracks, KeyTracks, OutputTime, PixelSize, Project, Range, Rect, SourceTime,
    TimeDelta, VideoColor,
};
use dc_ops::Evaluator;
use dc_render::golden::Golden;
use dc_render::{AdapterChoice, Bitmap, Compositor, Gpu, Layer, Nv12, Scene, TextMeasure};

pub type TestResult = Result<(), Box<dyn Error>>;

/// The fixture project whose `media/recording-1` holds real cursor pictures and telemetry.
pub fn clicks_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recordings/clicks.dcproj")
}

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

pub fn clip(project: &mut Project, path: &str, size: PixelSize, crop: Rect) -> TestResult {
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

pub fn size(width: u32, height: u32) -> PixelSize {
    PixelSize { width, height }
}

/// Renders the project's scene at output second `at` with bars as every video and pictures of
/// `clicks_fixture()` as sprites, and checks it against reference `name`.
pub fn check(name: &str, project: &Project, cursors: CursorTracks, at: f64) -> TestResult {
    let evaluator = Evaluator::new(project, cursors, KeyTracks::new(), &mut measure()?)?;
    let scene = evaluator.evaluate(OutputTime::from_secs_f64(at));
    render(name, project, &scene, &clicks_fixture())
}

/// [`check`] of a project without a cursor and with shortcuts `keys`, text in the fonts of
/// `.deps/fonts`.
pub fn check_keys(name: &str, project: &Project, keys: KeyTracks, at: f64) -> TestResult {
    let evaluator = Evaluator::new(project, CursorTracks::new(), keys, &mut measure()?)?;
    let scene = evaluator.evaluate(OutputTime::from_secs_f64(at));
    render(name, project, &scene, &clicks_fixture())
}

/// The folder `fixtures/` of the repository: pictures of image annotations are `images/…` there.
pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// [`check`] of a project without a cursor whose pictures are paths in [`fixtures`].
pub fn check_pictures(name: &str, project: &Project, at: f64) -> TestResult {
    let evaluator = Evaluator::new(
        project,
        CursorTracks::new(),
        KeyTracks::new(),
        &mut measure()?,
    )?;
    let scene = evaluator.evaluate(OutputTime::from_secs_f64(at));
    render(name, project, &scene, &fixtures())
}

/// The fonts of `.deps/fonts`, as the frames are drawn with.
fn measure() -> Result<TextMeasure, Box<dyn Error>> {
    let mut measure = TextMeasure::new();
    measure.load_fonts(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.deps/fonts"))?;
    Ok(measure)
}

fn render(name: &str, project: &Project, scene: &Scene, pictures: &Path) -> TestResult {
    let gpu = Gpu::new(AdapterChoice::Software)?;
    let mut compositor = Compositor::new(&gpu);
    compositor.load_fonts(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.deps/fonts"))?;
    for (_, layer) in scene.painted() {
        match layer {
            Layer::Video(video) => {
                let media = project
                    .clips
                    .iter()
                    .find(|c| c.media.path == video.source.media)
                    .ok_or("no clip of the video layer")?;
                let (y, uv) = bars(media.media.size);
                let nv12 = Nv12::new(media.media.size, &y, &uv, VideoColor::BT709)?;
                compositor.upload(&gpu, &video.source, nv12);
            }
            Layer::Sprite(sprite) => {
                let picture = dc_media::decode_picture(&pictures.join(&sprite.image.path))?;
                let frame = picture.frames.get(sprite.image.frame as usize);
                let frame = frame.ok_or("no such frame of the picture")?;
                let bitmap = Bitmap::new(picture.size, &frame.rgba)?;
                compositor.upload_image(&gpu, &sprite.image, bitmap)?;
            }
            _ => {}
        }
    }
    let image = compositor.render(&gpu, scene)?;
    Golden::new(&gpu)?
        .check(name, &image)
        .map_err(|error| error.to_string())?;
    Ok(())
}
