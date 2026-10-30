//! The graphics API of the app windows: the one backend of dc-render (decision 004).

use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu;

/// eframe's default list of backends took Vulkan on Windows. Closing a popover destroys its
/// Vulkan swapchain, which the NVIDIA driver builds on DXGI, while the GPU still uses its
/// images: the D3D12 debug layer of a debug build stops the process on that (the crash on the
/// «Звук» button). DX12 waits for the queue before releasing a swapchain.
pub fn options() -> WgpuConfiguration {
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = dc_render::BACKEND;
    // As in dc-render: the dxcompiler.dll of the Vulkan SDK on PATH breaks shaders on WARP.
    setup
        .instance_descriptor
        .backend_options
        .dx12
        .shader_compiler = wgpu::Dx12Compiler::Fxc;
    WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(setup),
        ..WgpuConfiguration::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_use_only_the_render_backend() {
        let WgpuSetup::CreateNew(setup) = options().wgpu_setup else {
            panic!("окна должны создавать свой wgpu");
        };
        assert_eq!(setup.instance_descriptor.backends, dc_render::BACKEND);
        #[cfg(windows)]
        assert!(
            !setup
                .instance_descriptor
                .backends
                .contains(wgpu::Backends::VULKAN)
        );
    }
}
