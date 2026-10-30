//! egui_kittest harness of our windows: our fonts and theme, and the FXC shader compiler for
//! WARP (as in dc-render: the dxcompiler.dll of the Vulkan SDK on PATH crashes WARP).

use egui_kittest::{Harness, HarnessBuilder};

pub fn harness<State>() -> HarnessBuilder<State> {
    Harness::<State>::builder()
        .wgpu_setup(wgpu_setup())
        .with_render_cursor(false)
        .with_context_setup(|ctx| {
            crate::install(ctx).unwrap_or_else(|e| panic!("тему для теста не поставить: {e}"));
        })
}

fn wgpu_setup() -> egui_wgpu::WgpuSetup {
    let mut setup = egui_kittest::wgpu::default_wgpu_setup();
    if let egui_wgpu::WgpuSetup::CreateNew(new) = &mut setup {
        new.instance_descriptor.backends = dc_render::BACKEND;
        new.instance_descriptor.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
    }
    setup
}
