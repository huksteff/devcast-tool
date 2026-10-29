// A rendered frame to NV12 for the encoder (decision 006): BT.709 limited range from the stored
// sRGB bytes (read through the plain view, so the values are the gamma-encoded R'G'B').
// Chroma is sited left like the recordings and the video pass read it: sample i sits on luma
// column 2i, filtered [1 2 1] / 4 across columns 2i-1..2i+1 and averaged over its two rows.
@group(0) @binding(0) var frame: texture_2d<f32>;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(corner * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
}

const KR: f32 = 0.2126;
const KB: f32 = 0.0722;

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(KR, 1.0 - KR - KB, KB));
}

fn texel(x: i32, y: i32) -> vec3<f32> {
    let last = vec2<i32>(textureDimensions(frame)) - 1;
    return textureLoad(frame, clamp(vec2<i32>(x, y), vec2<i32>(0), last), 0).rgb;
}

@fragment fn fs_y(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    return vec4<f32>((16.0 + 219.0 * luma(texel(p.x, p.y))) / 255.0, 0.0, 0.0, 1.0);
}

@fragment fn fs_uv(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy) * 2;
    var c = vec3<f32>(0.0);
    for (var row = 0; row < 2; row++) {
        c += texel(p.x - 1, p.y + row) + 2.0 * texel(p.x, p.y + row) + texel(p.x + 1, p.y + row);
    }
    c /= 8.0;
    let y = luma(c);
    let cb = (c.b - y) / (2.0 * (1.0 - KB));
    let cr = (c.r - y) / (2.0 * (1.0 - KR));
    return vec4<f32>((128.0 + 224.0 * cb) / 255.0, (128.0 + 224.0 * cr) / 255.0, 0.0, 1.0);
}
