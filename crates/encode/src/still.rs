//! Frames of a project rendered without a window into PNG: `devcast frame` and `devcast sheet`.
//! The same `Compositor::draw` as the preview and the export (ARCHITECTURE, decision 4); this
//! module only feeds it what a scene shows and saves the result.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::num::NonZeroU32;

use dc_media::{Decoder, FramePool, Picture, cached_probe, decode_picture, encode_png, nv12_len};
use dc_project::ProjectDir;
use dc_render::{Bitmap, Compositor, Gpu, Image, Layer, Nv12, Scene};

use crate::error::EncodeError;

/// Decoders of a project's media, kept open between frames, and its pictures decoded once: the
/// frames of an animated picture are uploaded one by one as the scenes show them.
pub struct Sources<'p> {
    project: &'p ProjectDir,
    decoders: HashMap<String, Decoder>,
    pictures: HashMap<String, Picture>,
}

impl<'p> Sources<'p> {
    pub fn new(project: &'p ProjectDir) -> Self {
        Self {
            project,
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
        for (_, layer) in scene.painted() {
            match layer {
                Layer::Video(video) => {
                    let decoder = self.decoder(&video.source.media)?;
                    let color = decoder.probe().color;
                    let frame = decoder.frame_at(video.source.time)?;
                    let nv12 = Nv12::new(frame.size, frame.y(), frame.uv(), color)?;
                    compositor.upload(gpu, &video.source, nv12);
                }
                Layer::Sprite(sprite) if !compositor.has_image(&sprite.image) => {
                    let key = &sprite.image;
                    let picture = match self.pictures.entry(key.path.clone()) {
                        Entry::Occupied(decoded) => decoded.into_mut(),
                        Entry::Vacant(slot) => {
                            slot.insert(decode_picture(&self.project.root().join(&key.path))?)
                        }
                    };
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
        match self.decoders.entry(media.to_owned()) {
            Entry::Occupied(open) => Ok(open.into_mut()),
            Entry::Vacant(slot) => {
                let probe = cached_probe(self.project, media)?;
                let pool = FramePool::new(nv12_len(probe.size));
                let path = self.project.root().join(media);
                Ok(slot.insert(Decoder::open(&path, probe, pool)?))
            }
        }
    }
}

/// `scene` as a PNG of its size.
pub fn frame_png(
    gpu: &Gpu,
    compositor: &mut Compositor,
    sources: &mut Sources<'_>,
    scene: &Scene,
) -> Result<Vec<u8>, EncodeError> {
    let image = render(gpu, compositor, sources, scene)?;
    Ok(encode_png(image.size, &image.rgba)?)
}

/// `scenes` side by side, `columns` in a row (`Image::sheet`), as one PNG.
pub fn sheet_png(
    gpu: &Gpu,
    compositor: &mut Compositor,
    sources: &mut Sources<'_>,
    scenes: &[Scene],
    columns: NonZeroU32,
) -> Result<Vec<u8>, EncodeError> {
    let frames = scenes
        .iter()
        .map(|scene| render(gpu, compositor, sources, scene))
        .collect::<Result<Vec<_>, _>>()?;
    let sheet = Image::sheet(&frames, columns)?;
    Ok(encode_png(sheet.size, &sheet.rgba)?)
}

fn render(
    gpu: &Gpu,
    compositor: &mut Compositor,
    sources: &mut Sources<'_>,
    scene: &Scene,
) -> Result<Image, EncodeError> {
    sources.feed(gpu, compositor, scene)?;
    Ok(compositor.render(gpu, scene)?)
}
