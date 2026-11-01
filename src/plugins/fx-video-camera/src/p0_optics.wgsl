// P0 optics and photons (cpu.rs p0_row): the scene through the lens at a sensor pixel, its mean
// signal as a fraction of the full well, 16-bit fixed in R,G.
// u0 = sensor table; u1 = (flags, bits(cir), bits(scene_ir), 0);
// f0 = (vig_t2, ca px, olpf px, a_r); f1 = (a_g, a_b, crosstalk, 0). T0 = the source.

fn lin(c: u32) -> f32 {
    let v = f32(c) * 0.003921569;
    if (v <= 0.04045) {
        return v * 0.07739938;
    }
    return pow((v + 0.055) * 0.9478673, 2.4);
}

fn scene(px: f32, py: f32, c: u32) -> f32 {
    let k = f32(KK());
    let d = fdims();
    let fx = px * k - 0.5;
    let fy = py * k - 0.5;
    let x0 = floor(fx);
    let y0 = floor(fy);
    let tx = fx - x0;
    let ty = fy - y0;
    let xa = clamp(i32(x0), 0, d.x - 1);
    let xb = clamp(i32(x0) + 1, 0, d.x - 1);
    let ya = clamp(i32(y0), 0, d.y - 1);
    let yb = clamp(i32(y0) + 1, 0, d.y - 1);
    let saa = lin(textureLoad(T0, vec2<i32>(xa, ya), 0)[c]);
    let sba = lin(textureLoad(T0, vec2<i32>(xb, ya), 0)[c]);
    let sab = lin(textureLoad(T0, vec2<i32>(xa, yb), 0)[c]);
    let sbb = lin(textureLoad(T0, vec2<i32>(xb, yb), 0)[c]);
    let top = saa + (sba - saa) * tx;
    let bot = sab + (sbb - sab) * tx;
    return top + (bot - top) * ty;
}

fn optics(sx: i32, sy: i32) -> vec3<f32> {
    let px = f32(sx) + 0.5;
    let py = f32(sy) + 0.5;
    if (((FL >> 12u) & 1u) == 0u) {
        return vec3<f32>(scene(px, py, 0u), scene(px, py, 1u), scene(px, py, 2u));
    }
    let s = sdims();
    let hw = f32(s.x) * 0.5;
    let hh = f32(s.y) * 0.5;
    let dx = px - hw;
    let dy = py - hh;
    let ca = P.f0.y / sqrt(hw * hw + hh * hh);
    let d = P.f0.z * 0.5;
    var out = vec3<f32>(0.0);
    for (var ch = 0u; ch < 3u; ch = ch + 1u) {
        let sc = 1.0 + ca * (1.0 - f32(ch));
        let qx = hw + dx * sc;
        let qy = hh + dy * sc;
        if (d > 0.0) {
            let a = scene(qx - d, qy - d, ch);
            let b = scene(qx + d, qy - d, ch);
            let e = scene(qx - d, qy + d, ch);
            let f = scene(qx + d, qy + d, ch);
            out[ch] = (a + b + e + f) * 0.25;
        } else {
            out[ch] = scene(qx, qy, ch);
        }
    }
    let t = 1.0 + P.f0.x * r2_of(sx, sy);
    let e = 1.0 / (t * t);
    return vec3<f32>(out.x * e, out.y * e, out.z * e);
}

fn photons(code: u32, s: vec3<f32>) -> f32 {
    let r = s.x;
    let g = s.y;
    let b = s.z;
    let x = P.f1.z;
    let o = 1.0 - x;
    let hx = x * 0.5;
    let ar = P.f0.w;
    let ag = P.f1.x;
    let ab = P.f1.y;
    let ir = bitcast<f32>(P.u1.z) * (0.5 * r + 0.35 * g + 0.15 * b);
    var vis = 0.0;
    switch code {
        case 0u: { vis = ar * (o * r + hx * (g + b)); }
        case 1u: { vis = ag * (o * g + hx * (r + b)); }
        case 2u: { vis = ab * (o * b + hx * (r + g)); }
        case 3u: { vis = ar * r + ag * g + ab * b; }
        default: { vis = 0.03 * (ar * r + ag * g + ab * b); }
    }
    return vis + bitcast<f32>(P.u1.y) * dye_nir(code) * ir;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    FL = P.u1.x;
    TAB = P.u0;
    let p = vec2<i32>(floor(pos.xy));
    let k = KK();
    let sx = p.x / k;
    let sy = p.y / k;
    let mu = photons(cfa_code(sx, sy), optics(sx, sy));
    let q = fix16(mu);
    return vec4<u32>(q >> 8u, q & 255u, 0u, 255u);
}
