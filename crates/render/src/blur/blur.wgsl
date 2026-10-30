// Separable gaussian blur in a scratch texture: box downsample of a part of the frame (or a glow
// mask drawn there), horizontal and vertical passes, then a composite back into the frame. All
// positions are pixels: `@builtin(position)` of the pass being drawn.
struct Params {
    area: vec4<f32>,    // xy: target px of the scratch's top-left; z: downsample d; w: sigma, texels
    texels: vec4<f32>,  // xy: used texels of the scratch (from its top-left)
    quad: vec4<f32>,    // composite quad, target px: left, top, right, bottom
    tint: vec4<f32>,    // linear rgb; a: how much the tint covers the blur
    region: vec4<f32>,  // rounded rectangle, target px: centre xy, half size zw
    frame: vec4<f32>,   // x: corner radius px; yz: target size px
    turn: vec4<f32>,    // xy: cos and sin of the region's turn, clockwise on screen; z: opacity
};
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var bilinear: sampler;
@group(0) @binding(2) var<uniform> p: Params;

fn corner(i: u32) -> vec2<f32> {
    return vec2<f32>(f32(i & 1u), f32(i >> 1u));
}

@vertex fn vs_full(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let c = corner(i) * 2.0 - 1.0;
    return vec4<f32>(c.x, -c.y, 0.0, 1.0);
}

@vertex fn vs_quad(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let px = mix(p.quad.xy, p.quad.zw, corner(i));
    let ndc = px / p.frame.yz * 2.0 - 1.0;
    return vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
}

fn sample_at(px: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(src, bilinear, px / vec2<f32>(textureDimensions(src)), 0.0);
}

// Scratch texel → the average of its d×d block of the frame. Each bilinear tap on a pixel corner
// averages 2×2 pixels, so the block is covered exactly.
@fragment fn fs_down(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let d = p.area.z;
    let n = max(floor(d / 2.0), 1.0);
    let step = d / n;
    let start = p.area.xy + floor(pos.xy) * d;
    var sum = vec4<f32>(0.0);
    for (var y = 0.0; y < n; y += 1.0) {
        for (var x = 0.0; x < n; x += 1.0) {
            sum += sample_at(start + (vec2<f32>(x, y) + 0.5) * step);
        }
    }
    return sum / (n * n);
}

// Taps at texel centres; outside the used texels the edge repeats (no dark rim at frame edges).
fn blur(pos: vec2<f32>, dir: vec2<f32>) -> vec4<f32> {
    let sigma = p.area.w;
    let r = ceil(sigma * 3.0);
    let last = p.texels.xy - 0.5;
    var sum = sample_at(pos);
    var total = 1.0;
    for (var i = 1.0; i <= r; i += 1.0) {
        let w = exp(-0.5 * (i / sigma) * (i / sigma));
        sum += (sample_at(clamp(pos + dir * i, vec2<f32>(0.5), last))
            + sample_at(clamp(pos - dir * i, vec2<f32>(0.5), last))) * w;
        total += 2.0 * w;
    }
    return sum / total;
}

@fragment fn fs_blur_h(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return blur(pos.xy, vec2<f32>(1.0, 0.0));
}

@fragment fn fs_blur_v(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return blur(pos.xy, vec2<f32>(0.0, 1.0));
}

// The blurred scratch at frame pixel `px`, bilinear between texels.
fn blurred(px: vec2<f32>) -> vec4<f32> {
    let at = clamp((px - p.area.xy) / p.area.z, vec2<f32>(0.5), p.texels.xy - 0.5);
    return sample_at(at);
}

@fragment fn fs_region(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let rgb = mix(blurred(pos.xy).rgb, p.tint.rgb, p.tint.a);
    let r = min(p.frame.x, min(p.region.z, p.region.w));
    // Into the region's own axes: turned back by its turn.
    let v = pos.xy - p.region.xy;
    let local = vec2<f32>(v.x * p.turn.x + v.y * p.turn.y, v.y * p.turn.x - v.x * p.turn.y);
    let q = abs(local) - p.region.zw + vec2<f32>(r);
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
    let a = clamp(0.5 - d, 0.0, 1.0) * p.turn.z;
    return vec4<f32>(rgb * a, a);
}

@fragment fn fs_glow(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return blurred(pos.xy);
}
