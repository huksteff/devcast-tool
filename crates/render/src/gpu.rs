//! The wgpu device shared by the preview window, export and the CLI.

use dc_core::PixelSize;

use crate::block::block_on;
use crate::error::GpuError;

/// Format of every frame and intermediate texture; egui shows it without a copy.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// View of [`FORMAT`] textures that passes render into and sample from (decision 004): the
/// hardware decodes sRGB to linear on read and encodes on write, so blending and filtering happen
/// in linear light while the stored bytes stay plain sRGB for PNG, egui and the NV12 export.
pub const SRGB_VIEW: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The native API of the OS, one per platform (decision 004). On Windows DX12: the reference
/// frames were made on it, WARP is a DX12 adapter, and decoded D3D frames reach it without a copy.
#[cfg(windows)]
pub const BACKEND: wgpu::Backends = wgpu::Backends::DX12;
#[cfg(target_os = "macos")]
pub const BACKEND: wgpu::Backends = wgpu::Backends::METAL;
#[cfg(not(any(windows, target_os = "macos")))]
pub const BACKEND: wgpu::Backends = wgpu::Backends::VULKAN;

/// Which adapter [`Gpu::new`] takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterChoice {
    /// A real GPU; the software adapter only when there is none.
    Auto,
    /// The software adapter (WARP on Windows): the reference frames of CI are rendered on it.
    Software,
}

/// Instance, adapter, device and queue. The window takes clones of them, so preview, export and
/// the CLI render on one device.
#[derive(Debug)]
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    pub fn new(choice: AdapterChoice) -> Result<Self, GpuError> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = BACKEND;
        let instance = wgpu::Instance::new(desc);
        let adapter = match choice {
            AdapterChoice::Auto => request(&instance, false).or_else(|_| request(&instance, true)),
            AdapterChoice::Software => request(&instance, true),
        }
        .map_err(GpuError::NoAdapter)?;
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("dc-render"),
            ..Default::default()
        }))
        .map_err(|source| GpuError::Device {
            adapter: adapter.get_info().name,
            source,
        })?;
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }

    /// Name, backend and kind of the adapter: reference frames are kept per backend.
    pub fn info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }

    /// Texture a pass renders into (through [`SRGB_VIEW`]) and the readback copies from.
    pub fn target(&self, size: PixelSize) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("target"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[SRGB_VIEW],
        })
    }
}

fn request(
    instance: &wgpu::Instance,
    software: bool,
) -> Result<wgpu::Adapter, wgpu::RequestAdapterError> {
    block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: software,
        ..Default::default()
    }))
}
