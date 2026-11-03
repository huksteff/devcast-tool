//! Frames of a project rendered without a window into PNG: `devcast frame` and `devcast sheet`.
//! The same `Compositor::draw` as the preview and the export (ARCHITECTURE, decision 4); this
//! module only feeds it what a scene shows and saves the result.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::num::NonZeroU32;

use dc_core::PixelSize;
use dc_media::{Decoder, FramePool, Picture, cached_probe, decode_picture, encode_png, nv12_len};
use dc_project::ProjectDir;
use dc_render::{Bitmap, Compositor, Gpu, Image, Layer, Nv12, Scene};

use crate::error::EncodeError;

/// Which video frame a scene gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seek {
    /// Exactly the frame shown at its time (export, `devcast frame`, the paused preview).
    Exact,
    /// The keyframe at or before it: fast, while the playhead is dragged (decision 003).
    Keyframe,
}

/// Feeds after which a decoder or a decoded picture no scene asked for is dropped: 2 s of
/// playback at 60 fps. Without it the preview keeps every video and picture it ever showed.
const UNUSED_FEEDS: u64 = 120;

/// Decoders of a project's media, kept open between frames, and its pictures decoded once: the
/// frames of an animated picture are uploaded one by one as the scenes show them. What the last
/// [`UNUSED_FEEDS`] feeds did not use is dropped.
pub struct Sources<'p> {
    project: &'p ProjectDir,
    /// Feeds so far: the clock of `Kept::used`.
    feeds: u64,
    decoders: HashMap<String, Kept<Decoder>>,
    pictures: HashMap<String, Kept<Picture>>,
}

/// A decoder or a picture and the feed that used it last.
struct Kept<T> {
    value: T,
    used: u64,
}

impl<'p> Sources<'p> {
    pub fn new(project: &'p ProjectDir) -> Self {
        Self {
            project,
            feeds: 0,
            decoders: HashMap::new(),
            pictures: HashMap::new(),
        }
    }

    /// Hands `compositor` the video frames `scene` shows and the pictures it does not hold yet.
    pub fn feed(
        &mut self,
        gpu: &Gpu,
        compositor: &mut Compositor,
        scene: &Scene,
    ) -> Result<(), EncodeError> {
        self.feed_at(gpu, compositor, scene, Seek::Exact)
    }

    /// [`Self::feed`] with the video frames picked by `seek`.
    pub fn feed_at(
        &mut self,
        gpu: &Gpu,
        compositor: &mut Compositor,
        scene: &Scene,
        seek: Seek,
    ) -> Result<(), EncodeError> {
        self.feeds += 1;
        let now = self.feeds;
        self.decoders
            .retain(|_, kept| now - kept.used < UNUSED_FEEDS);
        self.pictures
            .retain(|_, kept| now - kept.used < UNUSED_FEEDS);
        for (_, layer) in scene.painted() {
            match layer {
                Layer::Video(video) => {
                    let decoder = self.decoder(&video.source.media)?;
                    let color = decoder.probe().color;
                    let frame = match seek {
                        Seek::Exact => decoder.frame_at(video.source.time)?,
                        Seek::Keyframe => decoder.keyframe_at(video.source.time)?,
                    };
                    let nv12 = Nv12::new(frame.size, frame.y(), frame.uv(), color)?;
                    compositor.upload(gpu, &video.source, nv12);
                }
                Layer::Sprite(sprite) if !compositor.has_image(&sprite.image) => {
                    let key = &sprite.image;
                    let kept = match self.pictures.entry(key.path.clone()) {
                        Entry::Occupied(decoded) => decoded.into_mut(),
                        Entry::Vacant(slot) => slot.insert(Kept {
                            value: decode_picture(&self.project.root().join(&key.path))?,
                            used: now,
                        }),
                    };
                    kept.used = now;
                    let picture = &kept.value;
                    let frame = picture.frames.get(key.frame as usize).ok_or_else(|| {
                        EncodeError::NoPictureFrame {
                            path: key.path.clone(),
                            frame: key.frame,
                            frames: picture.frames.len(),
                        }
                    })?;
                    let bitmap = Bitmap::new(picture.size, &frame.rgba)?;
                    compositor.upload_image(gpu, key, bitmap)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn decoder(&mut self, media: &str) -> Result<&mut Decoder, EncodeError> {
        let kept = match self.decoders.entry(media.to_owned()) {
            Entry::Occupied(open) => open.into_mut(),
            Entry::Vacant(slot) => {
                let probe = cached_probe(self.project, media)?;
                let pool = FramePool::new(nv12_len(probe.size));
                let path = self.project.root().join(media);
                slot.insert(Kept {
                    value: Decoder::open(&path, probe, pool)?,
                    used: self.feeds,
                })
            }
        };
        kept.used = self.feeds;
        Ok(&mut kept.value)
    }
}

#[cfg(test)]
#[path = "still_tests.rs"]
mod tests;

/// `scene` scaled to `size` as a PNG.
pub fn frame_png(
    gpu: &Gpu,
    compositor: &mut Compositor,
    sources: &mut Sources<'_>,
    scene: &Scene,
    size: PixelSize,
) -> Result<Vec<u8>, EncodeError> {
    let image = render(gpu, compositor, sources, scene, size)?;
    Ok(encode_png(image.size, &image.rgba)?)
}

/// `scenes` scaled to `cell` side by side, `columns` in a row (`Image::sheet`), as one PNG.
pub fn sheet_png(
    gpu: &Gpu,
    compositor: &mut Compositor,
    sources: &mut Sources<'_>,
    scenes: &[Scene],
    columns: NonZeroU32,
    cell: PixelSize,
) -> Result<Vec<u8>, EncodeError> {
    let frames = scenes
        .iter()
        .map(|scene| render(gpu, compositor, sources, scene, cell))
        .collect::<Result<Vec<_>, _>>()?;
    let sheet = Image::sheet(&frames, columns)?;
    Ok(encode_png(sheet.size, &sheet.rgba)?)
}

fn render(
    gpu: &Gpu,
    compositor: &mut Compositor,
    sources: &mut Sources<'_>,
    scene: &Scene,
    size: PixelSize,
) -> Result<Image, EncodeError> {
    sources.feed(gpu, compositor, scene)?;
    Ok(compositor.render_at(gpu, scene, size)?)
}
