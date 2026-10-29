// Instanced SDF figures: rounded rectangle (0), ellipse (1), line (2), arrow (3).
// Fill: solid or 2-4 colour gradient (once or looping), outline, opacity, rotation; `fs_mask` draws
// what is visible of a figure in its glow colour, to be blurred.
// Colours arrive in sRGB with straight alpha and mix in sRGB like v1 and CSS gradients; the
// result is converted to linear light and premultiplied for blending.
struct Globals {
    view: vec4<f32>,    // xy: size of the drawn part of the frame px; z: frame px per target px
    origin: vec4<f32>,  // xy: top-left of the drawn part of the frame px
};
struct Shape {
    center_half: vec4<f32>,  // centre xy; half size of the bounds xy (px)
    transform: vec4<f32>,    // cos, sin of the rotation; outline width; opacity
    style: vec4<f32>,        // kind; fill colours (0 none, 1 solid, 2-4 gradient); looping; phase
    geometry: vec4<f32>,     // per kind, see shape_distance
    direction: vec4<f32>,    // xy: gradient direction; zw: padding
    colors: array<vec4<f32>, 4>,
    stroke: vec4<f32>,
    glow: vec4<f32>,         // glow colour
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> shapes: array<Shape>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) id: u32,
};

@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let s = shapes[ii];
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u)) * 2.0 - 1.0;
    let margin = s.transform.z * 0.5 + 2.0 * g.view.z;
    let local = corner * (s.center_half.zw + vec2<f32>(margin));
    let c = s.transform.x;
    let sn = s.transform.y;
    let world = s.center_half.xy + vec2<f32>(local.x * c - local.y * sn, local.x * sn + local.y * c);
    var out: VsOut;
    let p = (world - g.origin.xy) / g.view.xy;
    out.pos = vec4<f32>(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
    out.local = local;
    out.id = ii;
    return out;
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

fn shape_distance(s: Shape, p: vec2<f32>) -> f32 {
    let k = s.geometry;
    switch u32(s.style.x) {
        // Ellipse, k.xy: half axes. Exact for a circle, first-order elsewhere (outline width holds).
        case 1u: {
            let k0 = length(p / k.xy);
            let k1 = length(p / (k.xy * k.xy));
            return k0 * (k0 - 1.0) / k1;
        }
        // Line, k.x: half length, k.y: half width.
        case 2u: {
            return sd_segment(p, vec2<f32>(-k.x, 0.0), vec2<f32>(k.x, 0.0)) - k.y;
        }
        // Arrow from (-k.x, 0) to (k.x, 0), k.y: half width, k.z: whisker length.
        case 3u: {
            let tip = vec2<f32>(k.x, 0.0);
            let back = k.z * 0.70710677;
            let shaft = sd_segment(p, vec2<f32>(-k.x, 0.0), tip);
            let up = sd_segment(p, tip, tip + vec2<f32>(-back, -back));
            let down = sd_segment(p, tip, tip + vec2<f32>(-back, back));
            return min(shaft, min(up, down)) - k.y;
        }
        // Rounded rectangle, k.xy: half size, k.z: corner radius.
        default: {
            let r = min(k.z, min(k.x, k.y));
            let q = abs(p) - k.xy + vec2<f32>(r);
            return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
        }
    }
}

fn fill_color(id: u32, p: vec2<f32>) -> vec4<f32> {
    let s = shapes[id];
    let n = u32(s.style.y);
    if n == 1u {
        return shapes[id].colors[0];
    }
    let dir = s.direction.xy;
    let t = dot(p, dir) / dot(abs(dir), s.center_half.zw) * 0.5 + 0.5;
    if s.style.z > 0.5 {
        let x = fract(t + s.style.w) * f32(n);
        let i = u32(x) % n;
        return mix(shapes[id].colors[i], shapes[id].colors[(i + 1u) % n], fract(x));
    }
    let x = clamp(t, 0.0, 1.0) * f32(n - 1u);
    let i = min(u32(x), n - 2u);
    return mix(shapes[id].colors[i], shapes[id].colors[i + 1u], x - f32(i));
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

fn premultiplied(c: vec4<f32>, coverage: f32) -> vec4<f32> {
    let a = c.a * coverage;
    return vec4<f32>(srgb_to_linear(c.rgb) * a, a);
}

// Premultiplied colour of the figure at the fragment.
fn shade(v: VsOut) -> vec4<f32> {
    let s = shapes[v.id];
    let d = shape_distance(s, v.local);
    let aa = max(fwidth(d), 1e-3);
    var color = vec4<f32>(0.0);
    if s.style.y > 0.5 {
        color = premultiplied(fill_color(v.id, v.local), clamp(0.5 - d / aa, 0.0, 1.0));
    }
    if s.transform.z > 0.0 {
        let stroke = premultiplied(s.stroke, clamp(0.5 - (abs(d) - s.transform.z * 0.5) / aa, 0.0, 1.0));
        color = stroke + color * (1.0 - stroke.a);
    }
    return color * s.transform.w;
}

@fragment fn fs(v: VsOut) -> @location(0) vec4<f32> {
    return shade(v);
}

@fragment fn fs_mask(v: VsOut) -> @location(0) vec4<f32> {
    let glow = shapes[v.id].glow;
    return premultiplied(glow, shade(v).a);
}
