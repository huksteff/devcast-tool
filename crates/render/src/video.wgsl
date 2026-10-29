// Video pass: an NV12 frame drawn as a textured quad, YUV → R'G'B' by the file's matrix and range.
// With camera motion blur the quad covers the frame's place at both ends of the shutter, and each
// pixel averages the samples along the camera's path in float before its one write.
struct Video {
    rect: vec4<f32>,    // canvas px: x, y, w, h
    uv: vec4<f32>,      // source fractions: left, top, right, bottom (the crop)
    chroma: vec4<f32>,  // chroma fraction = luma fraction * xy + zw (siting)
    r: vec4<f32>,       // R' = dot((y, u, v, 1), r)
    g: vec4<f32>,
    b: vec4<f32>,
    camera: vec4<f32>,  // scale, x, y; frame px per target px
    back: vec4<f32>,    // camera at the shutter opening minus now: scale, x, y; samples
    quad: vec4<f32>,    // NDC: left, top, right, bottom
    shutter: vec4<f32>, // x: step of the samples back towards the opening
};
@group(0) @binding(0) var<uniform> video: Video;
@group(0) @binding(1) var luma: texture_2d<f32>;
@group(0) @binding(2) var chroma: texture_2d<f32>;
@group(0) @binding(3) var bilinear: sampler;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32(i & 1u), f32(i >> 1u));
    return vec4<f32>(mix(video.quad.xy, video.quad.zw, corner), 0.0, 1.0);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

fn rgb(uv: vec2<f32>) -> vec3<f32> {
    let y = textureSampleLevel(luma, bilinear, uv, 0.0).r;
    let c = textureSampleLevel(chroma, bilinear, uv * video.chroma.xy + video.chroma.zw, 0.0).rg;
    let yuv = vec4<f32>(y, c, 1.0);
    let rgb = vec3<f32>(dot(yuv, video.r), dot(yuv, video.g), dot(yuv, video.b));
    // The decoded R'G'B' is stored as is: the sRGB view encodes back what is decoded here.
    return srgb_to_linear(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)));
}

@fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = pos.xy * video.camera.w;
    let n = u32(video.back.w);
    var color = vec3<f32>(0.0);
    var cover = 0.0;
    for (var i = 0u; i < n; i++) {
        // Each sample from the exact current camera, so nothing piles up along the way.
        let camera = video.camera.xyz + video.back.xyz * (f32(n - 1u - i) * video.shutter.x);
        let f = ((p - camera.yz) / camera.x - video.rect.xy) / video.rect.zw;
        let inside = f32(all(f >= vec2<f32>(0.0)) && all(f < vec2<f32>(1.0)));
        color += rgb(mix(video.uv.xy, video.uv.zw, clamp(f, vec2<f32>(0.0), vec2<f32>(1.0)))) * inside;
        cover += inside;
    }
    return vec4<f32>(color, cover) / f32(n);
}
