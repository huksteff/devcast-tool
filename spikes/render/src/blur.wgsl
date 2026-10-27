// Separable gaussian blur helpers: copy (downsample a region), 1-D blur, composites.
struct Params {
    src_rect: vec4<f32>, // source uv rect for fs_copy: x0, y0, x1, y1
    step: vec4<f32>,     // xy: uv step between taps, z: sigma in texels
    tint: vec4<f32>,     // linear rgb, a = tint strength (region composite)
    rect: vec4<f32>,     // region size px (x, y), corner radius px (z), glow strength (w)
};
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> p: Params;

struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex fn vs_fullscreen(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VsOut;
    o.pos = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    o.uv = uv;
    return o;
}

@fragment fn fs_copy(v: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(src, samp, mix(p.src_rect.xy, p.src_rect.zw, v.uv), 0.0);
}

@fragment fn fs_blur(v: VsOut) -> @location(0) vec4<f32> {
    let sigma = max(p.step.z, 0.5);
    let r = i32(min(ceil(sigma * 3.0), 48.0));
    var acc = vec4<f32>(0.0);
    var total = 0.0;
    for (var i = -r; i <= r; i++) {
        let w = exp(-f32(i * i) / (2.0 * sigma * sigma));
        acc += textureSampleLevel(src, samp, v.uv + p.step.xy * f32(i), 0.0) * w;
        total += w;
    }
    return acc / total;
}

// Blurred region with tint and rounded corners, premultiplied output.
@fragment fn fs_region(v: VsOut) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(src, samp, v.uv, 0.0).rgb;
    let rgb = mix(c, p.tint.rgb, p.tint.a);
    let size = p.rect.xy;
    let r = p.rect.z;
    let q = abs(v.uv * size - size * 0.5) - (size * 0.5 - vec2<f32>(r));
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
    let a = clamp(0.5 - d, 0.0, 1.0);
    return vec4<f32>(rgb * a, a);
}

// Blurred glow texture (premultiplied) scaled by strength.
@fragment fn fs_glow(v: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(src, samp, v.uv, 0.0) * p.rect.w;
}

// Procedural "screen": gradient desktop with a window full of text-like lines.
fn hash(x: f32) -> f32 { return fract(sin(x * 127.1) * 43758.5453); }

@fragment fn fs_background(v: VsOut) -> @location(0) vec4<f32> {
    let uv = v.uv;
    var c = mix(vec3<f32>(0.03, 0.04, 0.10), vec3<f32>(0.12, 0.03, 0.14), uv.x + uv.y * 0.3);
    let win = uv.x > 0.08 && uv.x < 0.92 && uv.y > 0.08 && uv.y < 0.92;
    if win {
        c = vec3<f32>(0.85, 0.86, 0.88);
        if uv.y < 0.13 { c = vec3<f32>(0.10, 0.30, 0.65); }
        let line = floor((uv.y - 0.16) / 0.035);
        let in_line = fract((uv.y - 0.16) / 0.035) < 0.45 && uv.y > 0.16;
        let word = floor(uv.x * 60.0);
        let is_word = hash(word + line * 31.0) > 0.25 && uv.x < 0.2 + hash(line) * 0.7;
        if in_line && is_word { c = vec3<f32>(0.12, 0.12, 0.14); }
    }
    return vec4<f32>(c, 1.0);
}
