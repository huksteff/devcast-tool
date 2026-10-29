// Brush: lyon triangles of the strokes go into a 4× multisampled layer of their drawing
// (premultiplied, linear light); the resolved layer is then laid over the frame with the
// drawing's opacity.
struct StrokeOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex fn vs_stroke(@location(0) pos: vec2<f32>, @location(1) color: vec4<f32>) -> StrokeOut {
    var out: StrokeOut;
    out.pos = vec4<f32>(pos, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment fn fs_stroke(v: StrokeOut) -> @location(0) vec4<f32> {
    return v.color;
}

@group(0) @binding(0) var layer: texture_2d<f32>;
@group(0) @binding(1) var<uniform> opacity: vec4<f32>;

@vertex fn vs_layer(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32(i & 1u), f32(i >> 1u)) * 2.0 - 1.0;
    return vec4<f32>(corner, 0.0, 1.0);
}

@fragment fn fs_layer(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(layer, vec2<i32>(pos.xy), 0) * opacity.x;
}
