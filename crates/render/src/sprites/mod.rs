//! Sprite pass: uploaded pictures (image annotations, cursors, frames of animations) kept as
//! textures with mip chains, each sprite one quad.
//!
//! The texture cache holds every uploaded picture until the pictures take more than
//! [`IMAGE_BUDGET`]; then those the last drawn frame did not show go, oldest first.

mod mips;

use std::collections::HashMap;

use dc_core::PixelSize;

use crate::error::RenderError;
use crate::gpu::{Gpu, SRGB_VIEW};
use crate::pipeline::pipeline_with;
use crate::scene::{Camera, ImageKey, Sprite};
use crate::uniform::Slots;

use mips::Mips;

/// Memory for uploaded pictures (with their mip chains).
pub const IMAGE_BUDGET: u64 = 512 * 1024 * 1024;

/// Floats of `Sprite` of sprites.wgsl.
const FLOATS: usize = 16;

/// A picture as `dc_media::Picture` gives a frame of it: RGBA with straight alpha, rows without
/// padding.
#[derive(Debug, Clone, Copy)]
pub struct Bitmap<'a> {
    size: PixelSize,
    rgba: &'a [u8],
}

impl<'a> Bitmap<'a> {
    /// Checks that `rgba` holds a picture of `size`.
    pub fn new(size: PixelSize, rgba: &'a [u8]) -> Result<Self, RenderError> {
        let len = size.width as usize * size.height as usize * 4;
        if rgba.len() != len || len == 0 {
            return Err(RenderError::Bitmap {
                width: size.width,
                height: size.height,
                len: rgba.len(),
            });
        }
        Ok(Self { size, rgba })
    }
}

struct Picture {
    bind: wgpu::BindGroup,
    bytes: u64,
    /// The frame that last drew it (or the next one, for a fresh upload).
    used: u64,
}

pub(crate) struct SpritesPass {
    pipeline: wgpu::RenderPipeline,
    uniform_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    mips: Mips,
    slots: Slots<FLOATS>,
    slots_bind: wgpu::BindGroup,
    pictures: HashMap<ImageKey, Picture>,
    /// Counts drawn frames.
    clock: u64,
}

impl SpritesPass {
    pub(crate) fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let module = device.create_shader_module(wgpu::include_wgsl!("sprites.wgsl"));
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprites"),
            entries: &[Slots::<FLOATS>::layout_entry(
                0,
                wgpu::ShaderStages::VERTEX_FRAGMENT,
            )],
        });
        let texture_layout = texture_layout(device);
        let blend = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let layouts = [&uniform_layout, &texture_layout];
        let slots = Slots::new(gpu, "sprites");
        Self {
            pipeline: pipeline_with(device, &module, &layouts, ["vs", "fs"], blend),
            slots_bind: slots_bind(gpu, &uniform_layout, &slots),
            mips: Mips::new(gpu, &texture_layout),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("sprites"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            }),
            uniform_layout,
            texture_layout,
            slots,
            pictures: HashMap::new(),
            clock: 0,
        }
    }

    pub(crate) fn upload(
        &mut self,
        gpu: &Gpu,
        key: &ImageKey,
        bitmap: Bitmap<'_>,
    ) -> Result<(), RenderError> {
        let max = gpu.device.limits().max_texture_dimension_2d;
        if bitmap.size.width > max || bitmap.size.height > max {
            return Err(RenderError::BitmapTooBig {
                path: key.path.clone(),
                max,
            });
        }
        let texture = self.mips.texture(gpu, bitmap.size, bitmap.rgba);
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(SRGB_VIEW),
            ..Default::default()
        });
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let bytes = bitmap.rgba.len() as u64 * 4 / 3;
        let used = self.clock + 1;
        self.pictures
            .insert(key.clone(), Picture { bind, bytes, used });
        self.evict();
        Ok(())
    }

    pub(crate) fn contains(&self, key: &ImageKey) -> bool {
        self.pictures.contains_key(key)
    }

    /// Starts a frame.
    pub(crate) fn clear(&mut self) {
        self.clock += 1;
        self.slots.clear();
    }

    /// Places `sprite` seen through `camera` in a `frame`-sized scene drawn into `target`;
    /// returns the dynamic offset of its slot.
    pub(crate) fn push(
        &mut self,
        sprite: &Sprite,
        camera: Camera,
        frame: PixelSize,
        target: PixelSize,
    ) -> Result<u32, RenderError> {
        let picture = self
            .pictures
            .get_mut(&sprite.image)
            .ok_or_else(|| RenderError::NoImage {
                path: sprite.image.path.clone(),
                frame: sprite.image.frame,
            })?;
        picture.used = self.clock;
        let r = sprite.rect;
        let [cx, cy] = camera.apply([r.x + r.w / 2.0, r.y + r.h / 2.0]);
        let (half_w, half_h) = (r.w / 2.0 * camera.scale, r.h / 2.0 * camera.scale);
        let (sin, cos) = sprite.rotation.to_radians().sin_cos();
        let (fw, fh) = (frame.width as f32, frame.height as f32);
        let per_target = fw / target.width as f32;
        let (back, step, samples) = sprite.motion.map_or(([0.0; 2], 0.0, 1), |m| {
            let back = [
                (m.from[0] - r.x) * camera.scale,
                (m.from[1] - r.y) * camera.scale,
            ];
            (back, m.step(), m.samples.get())
        });
        let reach_x = (half_w * cos).abs() + (half_h * sin).abs() + per_target;
        let reach_y = (half_w * sin).abs() + (half_h * cos).abs() + per_target;
        let quad = [
            (cx - reach_x + back[0].min(0.0)) / fw * 2.0 - 1.0,
            1.0 - (cy - reach_y + back[1].min(0.0)) / fh * 2.0,
            (cx + reach_x + back[0].max(0.0)) / fw * 2.0 - 1.0,
            1.0 - (cy + reach_y + back[1].max(0.0)) / fh * 2.0,
        ];
        let rows = [
            [cx, cy, half_w, half_h],
            [cos, sin, sprite.opacity, per_target],
            [back[0], back[1], step, samples as f32],
            quad,
        ];
        let mut values = [0.0; FLOATS];
        values.copy_from_slice(rows.as_flattened());
        Ok(self.slots.push(&values))
    }

    pub(crate) fn upload_slots(&mut self, gpu: &Gpu) {
        if self.slots.upload(gpu) {
            self.slots_bind = slots_bind(gpu, &self.uniform_layout, &self.slots);
        }
    }

    pub(crate) fn record(&self, pass: &mut wgpu::RenderPass<'_>, image: &ImageKey, slot: u32) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.slots_bind, &[slot]);
        pass.set_bind_group(1, &self.pictures[image].bind, &[]);
        pass.draw(0..4, 0..1);
    }

    /// Drops pictures the last frame did not show, oldest first, while over the budget.
    fn evict(&mut self) {
        let sizes = self.pictures.iter().map(|(k, p)| (k, p.used, p.bytes));
        for key in stale(sizes, self.clock, IMAGE_BUDGET) {
            self.pictures.remove(&key);
        }
    }
}

/// Which of the pictures `(key, frame last used, bytes)` to drop to fit `budget`: only those
/// used before frame `clock`, the oldest first.
fn stale<'k>(
    pictures: impl Iterator<Item = (&'k ImageKey, u64, u64)>,
    clock: u64,
    budget: u64,
) -> Vec<ImageKey> {
    let mut all: Vec<_> = pictures.collect();
    let mut total: u64 = all.iter().map(|(_, _, bytes)| bytes).sum();
    all.retain(|(_, used, _)| *used < clock);
    all.sort_by_key(|(_, used, _)| *used);
    let mut drop = Vec::new();
    for (key, _, bytes) in all {
        if total <= budget {
            break;
        }
        drop.push(key.clone());
        total -= bytes;
    }
    drop
}

fn texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("sprite"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn slots_bind(gpu: &Gpu, layout: &wgpu::BindGroupLayout, slots: &Slots<FLOATS>) -> wgpu::BindGroup {
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sprites"),
        layout,
        entries: &[slots.bind_entry(0)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_drops_the_oldest_pictures_not_shown_by_the_last_frame() {
        let key = |n: u32| ImageKey {
            path: "a.gif".into(),
            frame: n,
        };
        let keys = [key(0), key(1), key(2), key(3)];
        // Frames last used: 3, 1, 5 (the last frame), 2; 40 bytes each, budget 80.
        let pictures = || keys.iter().zip([3, 1, 5, 2]).map(|(k, used)| (k, used, 40));
        assert_eq!(stale(pictures(), 5, 80), [key(1), key(3)]);
        assert_eq!(stale(pictures(), 5, 160), Vec::<ImageKey>::new());
        // Everything shown by the last frame stays even over the budget.
        assert_eq!(stale(pictures(), 1, 10), Vec::<ImageKey>::new());
    }
}
