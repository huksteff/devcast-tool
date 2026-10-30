// Instanced SDF figures: rounded rectangle (0), ellipse (1), line (2), arrow (3).
// Fill: solid or 2-4 colour gradient (once or looping), outline, opacity, rotation; `fs_mask` draws
// what is visible of a figure in its glow colour, to be blurred. Draw-in (`trim`) shows the path
// up to a length (trim.rs) with round ends and fades the fill of closed figures in.
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
    trim: vec4<f32>,         // draw-in: x: drawn path px; y: drawn part (1 whole)
    ends: vec4<f32>,         // draw-in: two ends of the drawn part (xy, zw)
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

const HALF_PI: f32 = 1.5707964;

// Rounded rectangle, half size `h`, radius `r`: px along the outline from the start of the top
// side clockwise to the outline point nearest `p` (trim.rs `rect_point` walks the same path).
fn rect_position(p: vec2<f32>, h: vec2<f32>, radius: f32) -> f32 {
    let r = max(min(radius, min(h.x, h.y)), 0.0);
    let i = h - vec2<f32>(r);
    let q = clamp(p, -i, i);
    var n = p - q;
    if n.x == 0.0 && n.y == 0.0 {
        let gap = h - abs(p);
        n = select(vec2<f32>(0.0, select(-1.0, 1.0, p.y > 0.0)), vec2<f32>(select(-1.0, 1.0, p.x > 0.0), 0.0), gap.x < gap.y);
    }
    let side = 2.0 * i;
    let arc = r * HALF_PI;
    if n.x >= 0.0 && n.y < 0.0 {
        return q.x + i.x + r * atan2(n.x, -n.y);
    }
    if n.x > 0.0 && n.y >= 0.0 {
        return side.x + arc + q.y + i.y + r * atan2(n.y, n.x);
    }
    if n.x <= 0.0 && n.y > 0.0 {
        return side.x + side.y + 2.0 * arc + i.x - q.x + r * atan2(-n.x, n.y);
    }
    return 2.0 * side.x + side.y + 3.0 * arc + i.y - q.y + r * atan2(-n.y, -n.x);
}

// Length of the ellipse arc from the top to parametric angle `x` (0..π/2), Simpson on 8 intervals.
fn ellipse_arc(a: f32, b: f32, x: f32) -> f32 {
    let h = x / 8.0;
    var sum = 0.0;
    for (var i = 0u; i <= 8u; i++) {
        let u = f32(i) * h;
        let weight = select(select(2.0, 4.0, (i & 1u) == 1u), 1.0, i == 0u || i == 8u);
        sum += weight * length(vec2<f32>(a * cos(u), b * sin(u)));
    }
    return sum * h / 3.0;
}

// Ellipse, half axes `k`: px along it from the top clockwise to the parametric angle of `p`.
fn ellipse_position(p: vec2<f32>, k: vec2<f32>) -> f32 {
    let a = max(k.x, 1e-3);
    let b = max(k.y, 1e-3);
    var t = atan2(p.x / a, -p.y / b);
    if t < 0.0 {
        t += 4.0 * HALF_PI;
    }
    let quarter = ellipse_arc(a, b, HALF_PI);
    let q = min(floor(t / HALF_PI), 3.0);
    let tau = t - q * HALF_PI;
    let within = select(quarter - ellipse_arc(a, b, HALF_PI - tau), ellipse_arc(a, b, tau), (u32(q) & 1u) == 0u);
    return q * quarter + within;
}

// Closed figures: px along the outline (trim.rs) to the point nearest `p`.
fn path_position(s: Shape, p: vec2<f32>) -> f32 {
    if u32(s.style.x) == 1u {
        return ellipse_position(p, s.geometry.xy);
    }
    return rect_position(p, s.geometry.xy, s.geometry.z);
}

// Lines and arrows: distance to the drawn part, `length` px of the shaft and then of both
// whiskers together; its ends stay round.
fn drawn_distance(s: Shape, p: vec2<f32>, length: f32) -> f32 {
    let k = s.geometry;
    let start = vec2<f32>(-k.x, 0.0);
    var d = sd_segment(p, start, start + vec2<f32>(min(length, 2.0 * k.x), 0.0));
    let whisker = min(length - 2.0 * k.x, k.z) * 0.70710677;
    if u32(s.style.x) == 3u && whisker > 0.0 {
        let tip = vec2<f32>(k.x, 0.0);
        d = min(d, min(
            sd_segment(p, tip, tip + vec2<f32>(-whisker, -whisker)),
            sd_segment(p, tip, tip + vec2<f32>(-whisker, whisker))));
    }
    return d - k.y;
}

// Premultiplied colour of the figure at the fragment.
fn shade(v: VsOut) -> vec4<f32> {
    let s = shapes[v.id];
    let body = shape_distance(s, v.local);
    // Draw-in: past the drawn length the figure is only its round ends.
    let trimmed = s.trim.y < 1.0;
    // Lines and arrows are painted by their fill, closed figures trim their outline.
    let open = u32(s.style.x) >= 2u;
    var d = body;
    var undrawn = false;
    if trimmed && open {
        d = drawn_distance(s, v.local, s.trim.x);
    } else if trimmed {
        undrawn = path_position(s, v.local) > s.trim.x;
    }
    let tip = min(distance(v.local, s.ends.xy), distance(v.local, s.ends.zw));
    let edge = select(abs(d), tip, undrawn);
    let aa = max(fwidth(d), 1e-3);
    let edge_aa = select(aa, max(fwidth(edge), 1e-3), trimmed);
    var color = vec4<f32>(0.0);
    if s.style.y > 0.5 {
        let fade = select(1.0, s.trim.y, trimmed && !open);
        color = premultiplied(fill_color(v.id, v.local), clamp(0.5 - d / aa, 0.0, 1.0) * fade);
    }
    if s.transform.z > 0.0 {
        let stroke = premultiplied(s.stroke, clamp(0.5 - (edge - s.transform.z * 0.5) / edge_aa, 0.0, 1.0));
        color = stroke + color * (1.0 - stroke.a);
    }
    return color * s.transform.w * select(1.0, 0.0, s.trim.y <= 0.0);
}

@fragment fn fs(v: VsOut) -> @location(0) vec4<f32> {
    return shade(v);
}

@fragment fn fs_mask(v: VsOut) -> @location(0) vec4<f32> {
    let glow = shapes[v.id].glow;
    return premultiplied(glow, shade(v).a);
}
