// Sprites: a picture with a mip chain (premultiplied, linear light through the sRGB view) drawn
// as a rotated quad with smooth edges. With motion blur each pixel averages the samples along the
// sprite's path in float before its one write.
struct Sprite {
    center_half: vec4<f32>,  // centre xy, half size zw, frame px
    turn: vec4<f32>,         // cos, sin of the rotation; opacity; frame px per target px
    trail: vec4<f32>,        // xy: way back to the shutter opening, frame px; z: step; w: samples
    quad: vec4<f32>,         // NDC: left, top, right, bottom
};
@group(0) @binding(0) var<uniform> s: Sprite;
@group(1) @binding(0) var picture: texture_2d<f32>;
@group(1) @binding(1) var trilinear: sampler;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32(i & 1u), f32(i >> 1u));
    return vec4<f32>(mix(s.quad.xy, s.quad.zw, corner), 0.0, 1.0);
}

@fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = pos.xy * s.turn.w;
    let half = s.center_half.zw;
    let n = u32(s.trail.w);
    var sum = vec4<f32>(0.0);
    for (var i = 0u; i < n; i++) {
        // Each sample from the exact current place, so nothing piles up along the way.
        let d = p - s.center_half.xy - s.trail.xy * (f32(n - 1u - i) * s.trail.z);
        let local = vec2<f32>(d.x * s.turn.x + d.y * s.turn.y, d.y * s.turn.x - d.x * s.turn.y);
        let uv = local / (2.0 * half) + 0.5;
        let q = abs(local) - half;
        let edge = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
        let coverage = clamp(0.5 - edge / s.turn.w, 0.0, 1.0);
        sum += textureSample(picture, trilinear, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0))) * coverage;
    }
    return sum * (s.turn.z / f32(n));
}
