//! Preview pictures for the recordings library and the recent projects list: the first frame of a
//! media file, fitted into [`PREVIEW_MAX`] without upscaling, as PNG in `cache/`.
//!
//! 320×180 is a 160×90 tile at 200% display scale: enough to recognise the screen, and small
//! (≈ 15 KB measured on 720p and 1080p test patterns), so a list of 16 projects or a library of
//! hundreds of recordings loads at once. PNG keeps small text and UI edges clean, where JPEG rings.
//!
//! The file name carries the media's size and modification time
//! (`video.mp4.preview-<bytes>-<ns>.png`): a changed media file gets a new preview and the old
//! one is deleted.

use std::path::{Path, PathBuf};

use dc_core::{PixelSize, Project, SourceTime};
use dc_project::{ProjectDir, atomic};
use ffmpeg_next as ffmpeg;

use crate::cache::{cached_probe, stamp};
use crate::decoder::{Decoder, Frame, nv12_len};
use crate::error::MediaError;
use crate::pool::FramePool;

pub const PREVIEW_MAX: PixelSize = PixelSize {
    width: 320,
    height: 180,
};

/// The preview of the project's media file `media` (path relative to the project, with `/`).
pub fn preview(project: &ProjectDir, media: &str) -> Result<PathBuf, MediaError> {
    let file = project.root().join(media);
    let stamp = stamp(&file)?;
    let mut target = project.cache_dir();
    target.extend(media.split('/'));
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let prefix = format!("{name}.preview-");
    let path = target.with_file_name(format!("{prefix}{}-{}.png", stamp.size, stamp.modified_ns));
    if path.is_file() {
        return Ok(path);
    }
    let png = render(project, media, &file)?;
    remove_old(&path, &prefix)?;
    atomic::write(&path, &png)?;
    Ok(path)
}

/// The preview of a project: of the first clip's media. `None` for a project without clips.
pub fn project_preview(dir: &ProjectDir, project: &Project) -> Result<Option<PathBuf>, MediaError> {
    project
        .clips
        .first()
        .map(|clip| preview(dir, &clip.media.path))
        .transpose()
}

/// `size` scaled down to fit [`PREVIEW_MAX`], keeping the aspect ratio; never scaled up.
pub fn fit(size: PixelSize) -> PixelSize {
    let scale = (f64::from(PREVIEW_MAX.width) / f64::from(size.width))
        .min(f64::from(PREVIEW_MAX.height) / f64::from(size.height))
        .min(1.0);
    let side = |pixels: u32| ((f64::from(pixels) * scale).round() as u32).max(1);
    PixelSize {
        width: side(size.width),
        height: side(size.height),
    }
}

fn render(project: &ProjectDir, media: &str, file: &Path) -> Result<Vec<u8>, MediaError> {
    let probe = cached_probe(project, media)?;
    let pool = FramePool::new(nv12_len(probe.size));
    let mut decoder = Decoder::open(file, probe, pool)?;
    let frame = decoder.frame_at(SourceTime::ZERO)?;
    let error = |source| MediaError::Preview {
        path: file.to_path_buf(),
        source,
    };
    let rgb = scale_to_rgb(&frame, fit(frame.size)).map_err(error)?;
    encode_png(&rgb).map_err(error)
}

/// NV12 (BT.709 limited range, decision 003) → RGB24 full range at `size`, area filter.
fn scale_to_rgb(frame: &Frame, size: PixelSize) -> Result<ffmpeg::frame::Video, ffmpeg::Error> {
    let (width, height) = (frame.size.width, frame.size.height);
    let mut nv12 = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::NV12, width, height);
    let uv_row = (width as usize).div_ceil(2) * 2;
    unpack_plane(frame.y(), width as usize, &mut nv12, 0);
    unpack_plane(frame.uv(), uv_row, &mut nv12, 1);
    let mut graph = ffmpeg::filter::Graph::new();
    let filter = |name| ffmpeg::filter::find(name).ok_or(ffmpeg::Error::FilterNotFound);
    let input = format!("video_size={width}x{height}:pix_fmt=nv12:time_base=1/1:pixel_aspect=1/1");
    graph.add(&filter("buffer")?, "in", &input)?;
    graph.add(&filter("buffersink")?, "out", "")?;
    let spec = format!(
        "scale={}:{}:flags=area:in_color_matrix=bt709:in_range=tv:out_range=pc,format=rgb24",
        size.width, size.height
    );
    graph.output("in", 0)?.input("out", 0)?.parse(&spec)?;
    graph.validate()?;
    let mut endpoint = |name| graph.get(name).ok_or(ffmpeg::Error::FilterNotFound);
    endpoint("in")?.source().add(&nv12)?;
    let mut rgb = ffmpeg::frame::Video::empty();
    endpoint("out")?.sink().frame(&mut rgb)?;
    Ok(rgb)
}

fn unpack_plane(source: &[u8], row_bytes: usize, frame: &mut ffmpeg::frame::Video, plane: usize) {
    let stride = frame.stride(plane);
    let data = frame.data_mut(plane);
    for (row, line) in source.chunks_exact(row_bytes).enumerate() {
        data[row * stride..row * stride + row_bytes].copy_from_slice(line);
    }
}

fn encode_png(rgb: &ffmpeg::frame::Video) -> Result<Vec<u8>, ffmpeg::Error> {
    let codec =
        ffmpeg::encoder::find(ffmpeg::codec::Id::PNG).ok_or(ffmpeg::Error::EncoderNotFound)?;
    let mut encoder = ffmpeg::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()?;
    encoder.set_width(rgb.width());
    encoder.set_height(rgb.height());
    encoder.set_format(ffmpeg::format::Pixel::RGB24);
    encoder.set_time_base((1, 1));
    let mut encoder = encoder.open_as(codec)?;
    encoder.send_frame(rgb)?;
    encoder.send_eof()?;
    let mut packet = ffmpeg::Packet::empty();
    encoder.receive_packet(&mut packet)?;
    packet
        .data()
        .map(<[u8]>::to_vec)
        .ok_or(ffmpeg::Error::InvalidData)
}

/// Deletes the previews of the same media made from an older version of the file.
fn remove_old(path: &Path, prefix: &str) -> Result<(), MediaError> {
    let Some(folder) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(folder).map_err(MediaError::io("создать папку", folder))?;
    let entries = std::fs::read_dir(folder).map_err(MediaError::io("прочитать папку", folder))?;
    for entry in entries {
        let old = entry
            .map_err(MediaError::io("прочитать папку", folder))?
            .path();
        let name = old.file_name().map(|n| n.to_string_lossy().into_owned());
        if name.is_some_and(|n| n.starts_with(prefix) && n.ends_with(".png")) {
            std::fs::remove_file(&old).map_err(MediaError::io("удалить", &old))?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
