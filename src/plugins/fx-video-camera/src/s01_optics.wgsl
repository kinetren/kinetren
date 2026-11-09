// P0 the lens and the photons (cpu.rs optics_row): distortion and lateral CA per channel (the field
// fitted to the scene, lens_fit), the
// 4-spot OLPF, cos⁴ vignetting and colour shading over the unprocessed scene, then the mean signal
// of each sensor pixel's filter as a fraction of the full well, 16-bit fixed.
// u0 = sensor table; u1 = (flags, bits(cir), bits(scene_ir), bits(k_lens));
// f0 = (vig_t2, ca px, olpf px, a_r); f1 = (a_g, a_b, crosstalk, colour shading). T0 = U.

fn scene(px: f32, py: f32, c: u32) -> f32 {
    let k = f32(KK());
    let fx = px * k - 0.5;
    let fy = py * k - 0.5;
    let x0 = floor(fx);
    let y0 = floor(fy);
    let tx = fx - x0;
    let ty = fy - y0;
    let xa = i32(x0);
    let ya = i32(y0);
    let saa = unpack_scene(at_frame(xa, ya))[c];
    let sba = unpack_scene(at_frame(xa + 1, ya))[c];
    let sab = unpack_scene(at_frame(xa, ya + 1))[c];
    let sbb = unpack_scene(at_frame(xa + 1, ya + 1))[c];
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
    let hd2 = hw * hw + hh * hh;
    let dist = 1.0 + bitcast<f32>(P.u1.w) * ((dx * dx + dy * dy) / hd2);
    let ca = P.f0.y / sqrt(hd2);
    let fit = lens_fit(bitcast<f32>(P.u1.w), ca, border_r2_min(hw, hh, hd2));
    let d = P.f0.z * 0.5;
    var out = vec3<f32>(0.0);
    for (var ch = 0u; ch < 3u; ch = ch + 1u) {
        let sc = dist * (1.0 + ca * (1.0 - f32(ch))) / fit;
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
    let r2 = r2_of(sx, sy);
    let t = 1.0 + P.f0.x * r2;
    let e = 1.0 / (t * t);
    let cs = P.f1.w;
    return vec3<f32>(out.x * e * (1.0 - cs * r2), out.y * e, out.z * e * (1.0 + 0.5 * cs * r2));
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
    begin(P.u1.x);
    TAB = P.u0;
    let p = vec2<i32>(floor(pos.xy));
    let k = KO();
    let sx = p.x / k;
    let sy = p.y / k;
    return t16(fix16(photons(cfa_code(sx, sy), optics(sx, sy))));
}
