//! The graphics device of the app: one `dc_render::Gpu` that the windows and the editor's
//! preview share (render CLAUDE.md: the window takes its clones and makes no device of its own),
//! so the compositor's frame is an egui texture without a copy.

use std::sync::Arc;

use dc_render::{AdapterChoice, Gpu};
use eframe::egui::mutex::RwLock;
use eframe::egui_wgpu::{RenderState, Renderer, WgpuConfiguration, WgpuSetup, WgpuSetupExisting};

use crate::error::AppError;

/// The device of dc-render on its one backend (decision 004). eframe's default list of backends
/// took Vulkan on Windows. Closing a popover destroys its Vulkan swapchain, which the NVIDIA
/// driver builds on DXGI, while the GPU still uses its images: the D3D12 debug layer of a debug
/// build stops the process on that (the crash on the «Звук» button). DX12 waits for the queue
/// before releasing a swapchain.
pub fn device() -> Result<Gpu, AppError> {
    let instance = eframe::wgpu::Instance::new(instance_descriptor());
    Gpu::on(instance, AdapterChoice::Auto).map_err(|e| AppError::Window(e.to_string()))
}

/// dc-render's instance whose DX12 windows present through a DirectComposition visual: its
/// swapchain has premultiplied alpha, so the transparent HUD, popovers and countdown show the
/// desktop around their rounded panel (decision 020; with a swapchain of the window handle the
/// see-through pixels were black).
fn instance_descriptor() -> eframe::wgpu::InstanceDescriptor {
    let mut desc = dc_render::instance_descriptor();
    desc.backend_options.dx12.presentation_system = eframe::wgpu::Dx12SwapchainKind::DxgiFromVisual;
    desc
}

/// eframe draws the windows with clones of `gpu`.
pub fn options(gpu: &Gpu) -> WgpuConfiguration {
    WgpuConfiguration {
        wgpu_setup: WgpuSetup::Existing(WgpuSetupExisting {
            instance: gpu.instance.clone(),
            adapter: gpu.adapter.clone(),
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
        }),
        ..WgpuConfiguration::default()
    }
}

/// The device and egui's renderer of the windows: the preview draws into textures of the
/// device and registers them with the renderer.
#[derive(Clone)]
pub struct PreviewGpu {
    pub gpu: Arc<Gpu>,
    pub renderer: Arc<RwLock<Renderer>>,
}

impl PreviewGpu {
    pub fn new(gpu: Gpu, state: &RenderState) -> Self {
        Self {
            gpu: Arc::new(gpu),
            renderer: Arc::clone(&state.renderer),
        }
    }
}

impl std::fmt::Debug for PreviewGpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreviewGpu")
            .field("adapter", &self.gpu.info().name)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_use_the_render_device() {
        let gpu = Gpu::new(AdapterChoice::Software).unwrap();
        let WgpuSetup::Existing(setup) = options(&gpu).wgpu_setup else {
            panic!("окна должны брать устройство dc-render");
        };
        assert_eq!(setup.device, gpu.device);
        assert_eq!(setup.adapter.get_info().backend, gpu.info().backend);
        #[cfg(windows)]
        assert_eq!(gpu.info().backend, eframe::wgpu::Backend::Dx12);
    }

    /// The transparent windows need the swapchain of a composition visual (decision 020).
    #[test]
    fn windows_present_through_a_composition_visual() {
        let desc = instance_descriptor();
        assert_eq!(
            desc.backend_options.dx12.presentation_system,
            eframe::wgpu::Dx12SwapchainKind::DxgiFromVisual
        );
        assert_eq!(desc.backends, dc_render::BACKEND);
    }
}
