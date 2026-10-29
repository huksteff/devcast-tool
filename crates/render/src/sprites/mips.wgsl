// The mip chain of a sprite picture, drawn level by level through the sRGB view (linear light).
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var bilinear: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    let corner = vec2<f32>(f32(i & 1u), f32(i >> 1u));
    var out: VsOut;
    out.pos = vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
    out.uv = corner;
    return out;
}

// Straight alpha → premultiplied, so that averaging keeps colours of transparent edges.
@fragment fn fs_premultiply(v: VsOut) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(source, bilinear, v.uv, 0.0);
    return vec4<f32>(c.rgb * c.a, c.a);
}

// One level from the one above: the bilinear tap in the middle of each 2×2 block.
@fragment fn fs_down(v: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(source, bilinear, v.uv, 0.0);
}
