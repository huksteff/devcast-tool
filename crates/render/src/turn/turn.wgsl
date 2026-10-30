// Turned text: the upright layer laid over the frame turned clockwise around the box centre.
// Premultiplied, linear light (the layer is read through the sRGB view).
struct Turn {
    // xy: centre of the box, target px; zw: cos and sin of the turn.
    centre: vec4<f32>,
    // xy: upright target px of layer texel (0, 0).
    origin: vec4<f32>,
    // Left, top, right, bottom of the upright part this tile shows, target px.
    area: vec4<f32>,
};

@group(0) @binding(0) var layer: texture_2d<f32>;
@group(0) @binding(1) var bilinear: sampler;
@group(0) @binding(2) var<uniform> turn: Turn;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32(i & 1u), f32(i >> 1u)) * 2.0 - 1.0;
    return vec4<f32>(corner, 0.0, 1.0);
}

@fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let d = pos.xy - turn.centre.xy;
    let c = turn.centre.z;
    let s = turn.centre.w;
    // The inverse of the clockwise turn (y down): where this pixel was before turning.
    let upright = turn.centre.xy + vec2<f32>(c * d.x + s * d.y, c * d.y - s * d.x);
    if any(upright < turn.area.xy) || any(upright >= turn.area.zw) {
        return vec4<f32>(0.0);
    }
    let size = vec2<f32>(textureDimensions(layer));
    return textureSampleLevel(layer, bilinear, (upright - turn.origin.xy) / size, 0.0);
}
