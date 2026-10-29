// Video pass: an NV12 frame (BT.709, limited range, chroma sited left as in H.264) drawn as a
// textured quad.
struct Video {
    rect: vec4<f32>,    // NDC: left, top, right, bottom
    uv: vec4<f32>,      // source fractions: left, top, right, bottom (the crop)
    chroma: vec4<f32>,  // chroma fraction = luma fraction * xy + (z, 0); w: padding
};
@group(0) @binding(0) var<uniform> video: Video;
@group(0) @binding(1) var luma: texture_2d<f32>;
@group(0) @binding(2) var chroma: texture_2d<f32>;
@group(0) @binding(3) var bilinear: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    let corner = vec2<f32>(f32(i & 1u), f32(i >> 1u));
    var out: VsOut;
    out.pos = vec4<f32>(mix(video.rect.xy, video.rect.zw, corner), 0.0, 1.0);
    out.uv = mix(video.uv.xy, video.uv.zw, corner);
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

@fragment fn fs(v: VsOut) -> @location(0) vec4<f32> {
    let y = (textureSample(luma, bilinear, v.uv).r * 255.0 - 16.0) / 219.0;
    let c = (textureSample(chroma, bilinear, v.uv * video.chroma.xy + vec2<f32>(video.chroma.z, 0.0)).rg * 255.0 - 128.0) / 224.0;
    let rgb = vec3<f32>(
        y + 1.5748 * c.y,
        y - 0.187324 * c.x - 0.468124 * c.y,
        y + 1.8556 * c.x,
    );
    // The decoded R'G'B' is stored as is: the sRGB view encodes back what is decoded here.
    return vec4<f32>(srgb_to_linear(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
