// Instanced SDF shapes: rounded rect (0), ellipse (1), arrow (2).
// Fill: solid or 2-4 stop gradient (static or animated, seamless), stroke, opacity, rotation.
struct Globals { viewport: vec2<f32>, time: f32, pad: f32 };
struct Shape {
    center_half: vec4<f32>,   // center x, y; half size x, y (px)
    params: vec4<f32>,        // rotation (rad), corner radius, stroke width, opacity
    kind: vec4<u32>,          // kind, stop count, filled (0/1), animated (0/1)
    grad: vec4<f32>,          // direction x, y; animation speed (cycles/s); unused
    stop_pos: vec4<f32>,
    colors: array<vec4<f32>, 4>,
    stroke_color: vec4<f32>,
    glow_color: vec4<f32>,    // alpha 0 = no glow
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
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let margin = s.params.z * 0.5 + 2.0;
    let local = corners[vi] * (s.center_half.zw + vec2<f32>(margin));
    let c = cos(s.params.x);
    let sn = sin(s.params.x);
    let world = s.center_half.xy + vec2<f32>(local.x * c - local.y * sn, local.x * sn + local.y * c);
    var o: VsOut;
    o.pos = vec4<f32>(world.x / g.viewport.x * 2.0 - 1.0, 1.0 - world.y / g.viewport.y * 2.0, 0.0, 1.0);
    o.local = local;
    o.id = ii;
    return o;
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

fn sd_triangle(p: vec2<f32>, p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>) -> f32 {
    let e0 = p1 - p0; let e1 = p2 - p1; let e2 = p0 - p2;
    let v0 = p - p0; let v1 = p - p1; let v2 = p - p2;
    let pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    let pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    let pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
    let s = sign(e0.x * e2.y - e0.y * e2.x);
    let d = min(min(vec2<f32>(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x)),
                    vec2<f32>(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x))),
                    vec2<f32>(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x)));
    return -sqrt(d.x) * sign(d.y);
}

fn shape_distance(s: Shape, p: vec2<f32>) -> f32 {
    let h = s.center_half.zw;
    switch s.kind.x {
        case 1u: {
            let k0 = length(p / h);
            let k1 = length(p / (h * h));
            return k0 * (k0 - 1.0) / max(k1, 1e-5);
        }
        case 2u: {
            let head = min(h.y * 2.2, h.x);
            let shaft = sd_segment(p, vec2<f32>(-h.x, 0.0), vec2<f32>(h.x - head * 0.9, 0.0)) - h.y * 0.32;
            let tip = sd_triangle(p, vec2<f32>(h.x, 0.0), vec2<f32>(h.x - head, h.y), vec2<f32>(h.x - head, -h.y));
            return min(shaft, tip);
        }
        default: {
            let r = min(s.params.y, min(h.x, h.y));
            let q = abs(p) - h + vec2<f32>(r);
            return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
        }
    }
}

fn fill_color(id: u32, p: vec2<f32>) -> vec4<f32> {
    let s = shapes[id];
    let n = max(s.kind.y, 1u);
    if n == 1u { return shapes[id].colors[0]; }
    let h = s.center_half.zw;
    var t = dot(p, s.grad.xy) / max(dot(abs(s.grad.xy), h), 1e-5) * 0.5 + 0.5;
    if s.kind.w == 1u {
        // Animated: colours flow along the direction and wrap seamlessly (last → first).
        let x = fract(t * 0.5 - g.time * s.grad.z) * f32(n);
        let i = u32(floor(x)) % n;
        let j = (i + 1u) % n;
        return mix(shapes[id].colors[i], shapes[id].colors[j], smoothstep(0.0, 1.0, fract(x)));
    }
    t = clamp(t, 0.0, 1.0);
    var c = shapes[id].colors[0];
    for (var i = 1u; i < n; i++) {
        let a = s.stop_pos[i - 1u];
        let b = s.stop_pos[i];
        c = mix(c, shapes[id].colors[i], clamp((t - a) / max(b - a, 1e-5), 0.0, 1.0));
    }
    return c;
}

@fragment fn fs_main(v: VsOut) -> @location(0) vec4<f32> {
    let s = shapes[v.id];
    let d = shape_distance(s, v.local);
    let aa = max(fwidth(d), 1e-3);
    let fill = select(0.0, clamp(0.5 - d / aa, 0.0, 1.0), s.kind.z == 1u);
    let stroke = select(0.0, clamp(0.5 - (abs(d) - s.params.z * 0.5) / aa, 0.0, 1.0), s.params.z > 0.0);
    let fc = fill_color(v.id, v.local);
    let fa = fc.a * fill;
    let sa = s.stroke_color.a * stroke;
    let col = vec4<f32>(s.stroke_color.rgb * sa, sa) + vec4<f32>(fc.rgb * fa, fa) * (1.0 - sa);
    return col * s.params.w;
}

// Mask of what is actually visible (fill and/or stroke) in the glow colour; blurred afterwards.
@fragment fn fs_glow_mask(v: VsOut) -> @location(0) vec4<f32> {
    let s = shapes[v.id];
    let d = shape_distance(s, v.local);
    let aa = max(fwidth(d), 1e-3);
    let fill = select(0.0, clamp(0.5 - d / aa, 0.0, 1.0), s.kind.z == 1u);
    let stroke = select(0.0, clamp(0.5 - (abs(d) - s.params.z * 0.5) / aa, 0.0, 1.0), s.params.z > 0.0);
    let a = s.glow_color.a * max(fill, stroke) * s.params.w;
    return vec4<f32>(s.glow_color.rgb * a, a);
}
