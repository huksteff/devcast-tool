// Sprites: a picture with a mip chain (premultiplied, linear light through the sRGB view) drawn
// as a rotated quad with smooth edges.
struct Sprite {
    center_half: vec4<f32>,  // centre xy, half size zw, frame px
    turn: vec4<f32>,         // cos, sin of the rotation; opacity; frame px per target px
    frame: vec4<f32>,        // xy: frame size px
};
@group(0) @binding(0) var<uniform> s: Sprite;
@group(1) @binding(0) var picture: texture_2d<f32>;
@group(1) @binding(1) var trilinear: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
};

@vertex fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    let corner = vec2<f32>(f32(i & 1u), f32(i >> 1u)) * 2.0 - 1.0;
    let local = corner * (s.center_half.zw + vec2<f32>(s.turn.w));
    let c = s.turn.x;
    let sn = s.turn.y;
    let world = s.center_half.xy + vec2<f32>(local.x * c - local.y * sn, local.x * sn + local.y * c);
    var out: VsOut;
    out.pos = vec4<f32>(world.x / s.frame.x * 2.0 - 1.0, 1.0 - world.y / s.frame.y * 2.0, 0.0, 1.0);
    out.local = local;
    return out;
}

@fragment fn fs(v: VsOut) -> @location(0) vec4<f32> {
    let half = s.center_half.zw;
    let uv = v.local / (2.0 * half) + 0.5;
    let q = abs(v.local) - half;
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
    let coverage = clamp(0.5 - d / s.turn.w, 0.0, 1.0);
    return textureSample(picture, trilinear, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0))) * coverage * s.turn.z;
}
