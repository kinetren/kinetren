// U: the unprocessing (cpu.rs unprocess_row), at FRAME pixels: the source's transfer undone, the
// defocus disc, the source's saturation matrix and white balance undone → the scene, packed.
// u0 = (flags, bits(m8), bits(1/wb_r), bits(1/wb_b)); u1 = (bits(blur px), the curve's table is in T2, 0, 0);
// f0, f1 = m0..m7. T0 = the source, T2 = s12's table when u1.y is 1.

// the source's transfer of a channel byte: `decode` (camera_common.wgsl) or, with defocus, its
// table (s12_curve.wgsl, per-sample-costs row 362: 51 `pow` a pixel), read as the f32's four bytes
fn decode_at(c: u32, curve: u32) -> f32 {
    if (P.u1.y == 1u) {
        let t = textureLoad(T2, vec2<i32>(i32(c), 0), 0);
        return bitcast<f32>(t.r | (t.g << 8u) | (t.b << 16u) | (t.a << 24u));
    }
    return decode(c, curve);
}

fn lin(x: i32, y: i32, curve: u32) -> vec3<f32> {
    let t = at_frame(x, y);
    return vec3<f32>(decode_at(t.r, curve), decode_at(t.g, curve), decode_at(t.b, curve));
}

const DISC = array<vec2<f32>, 8>(
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.70710678, 0.70710678),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(-0.70710678, 0.70710678),
    vec2<f32>(-1.0, 0.0),
    vec2<f32>(-0.70710678, -0.70710678),
    vec2<f32>(0.0, -1.0),
    vec2<f32>(0.70710678, -0.70710678),
);

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    begin(P.u0.x);
    let p = vec2<i32>(floor(pos.xy));
    let curve = (FL >> 17u) & 7u;
    var s = lin(p.x, p.y, curve);
    let blur = bitcast<f32>(P.u1.x);
    if (blur > 0.0) {
        for (var ring = 0; ring < 2; ring = ring + 1) {
            var r = blur;
            if (ring == 0) {
                r = 0.6 * blur;
            }
            for (var i = 0; i < 8; i = i + 1) {
                let d = DISC[i];
                let ox = i32(floor(d.x * r + 0.5));
                let oy = i32(floor(d.y * r + 0.5));
                let q = lin(p.x + ox, p.y + oy, curve);
                s = vec3<f32>(s.x + q.x, s.y + q.y, s.z + q.z);
            }
        }
        let inv = 0.05882353;
        s = vec3<f32>(s.x * inv, s.y * inv, s.z * inv);
    }
    let r = (P.f0.x * s.x + P.f0.y * s.y + P.f0.z * s.z) * bitcast<f32>(P.u0.z);
    let g = P.f0.w * s.x + P.f1.x * s.y + P.f1.y * s.z;
    let b = (P.f1.z * s.x + P.f1.w * s.y + bitcast<f32>(P.u0.y) * s.z) * bitcast<f32>(P.u0.w);
    let qr = u32(floor(sqrt(clamp(r, 0.0, 2.0) * 0.5) * 2047.0 + 0.5));
    let qg = u32(floor(sqrt(clamp(g, 0.0, 2.0) * 0.5) * 2047.0 + 0.5));
    let qb = u32(floor(sqrt(clamp(b, 0.0, 2.0) * 0.5) * 1023.0 + 0.5));
    let packed = qr | (qg << 11u) | (qb << 22u);
    return vec4<u32>(packed & 255u, (packed >> 8u) & 255u, (packed >> 16u) & 255u, packed >> 24u);
}
