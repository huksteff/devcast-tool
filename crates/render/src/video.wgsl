// Video pass: an NV12 frame drawn as a textured quad, YUV → R'G'B' by the file's matrix and range.
struct Video {
    rect: vec4<f32>,    // NDC: left, top, right, bottom
    uv: vec4<f32>,      // source fractions: left, top, right, bottom (the crop)
    chroma: vec4<f32>,  // chroma fraction = luma fraction * xy + zw (siting)
    r: vec4<f32>,       // R' = dot((y, u, v, 1), r)
    g: vec4<f32>,
    b: vec4<f32>,
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
    let y = textureSample(luma, bilinear, v.uv).r;
    let c = textureSample(chroma, bilinear, v.uv * video.chroma.xy + video.chroma.zw).rg;
    let yuv = vec4<f32>(y, c, 1.0);
    let rgb = vec3<f32>(dot(yuv, video.r), dot(yuv, video.g), dot(yuv, video.b));
    // The decoded R'G'B' is stored as is: the sRGB view encodes back what is decoded here.
    return vec4<f32>(srgb_to_linear(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
