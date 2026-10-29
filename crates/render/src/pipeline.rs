//! Render pipelines of the passes: a quad strip of 4 vertices drawn into [`SRGB_VIEW`].

use crate::gpu::SRGB_VIEW;

/// A pipeline of `module` with entry points `vs` and `fs` and one bind group of `layout`.
pub(crate) fn pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
    stages: [&str; 2],
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    pipeline_with(device, module, &[layout], stages, blend)
}

/// [`pipeline`] with bind groups of `layouts` in order.
pub(crate) fn pipeline_with(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layouts: &[&wgpu::BindGroupLayout],
    [vs, fs]: [&str; 2],
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    let layouts: Vec<_> = layouts.iter().copied().map(Some).collect();
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(fs),
        bind_group_layouts: &layouts,
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fs),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some(vs),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(fs),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: SRGB_VIEW,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}
