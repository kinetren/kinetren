// P3b lens correction (cpu.rs lens_correct_row): dewarp and lateral CA correction — per channel, the
// captured radius x with x(1 + k x²)(1 + ca_ch) = r_o by three Newton steps, then the captured
// picture bilinear there (in sensor units). u0 = (flags, 0, 0, 0); f0 = (k_corr, ca_corr px, 0, 0).
// T0 = P3, T1 = the source.

fn rgb_at(px: f32, py: f32, ch: u32) -> f32 {
    let fx = px - 0.5;
    let fy = py - 0.5;
    let x0 = floor(fx);
    let y0 = floor(fy);
    let tx = fx - x0;
    let ty = fy - y0;
    let xa = i32(x0);
    let ya = i32(y0);
    let saa = f32(at(xa, ya)[ch]);
    let sba = f32(at(xa + 1, ya)[ch]);
    let sab = f32(at(xa, ya + 1)[ch]);
    let sbb = f32(at(xa + 1, ya + 1)[ch]);
    let top = saa + (sba - saa) * tx;
    let bot = sab + (sbb - sab) * tx;
    return top + (bot - top) * ty;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    FL = P.u0.x;
    let p = vec2<i32>(floor(pos.xy));
    let k = KK();
    let sx = p.x / k;
    let sy = p.y / k;
    let s = sdims();
    let hw = f32(s.x) * 0.5;
    let hh = f32(s.y) * 0.5;
    let hd2 = hw * hw + hh * hh;
    let ca = P.f0.y / sqrt(hd2);
    let kc = P.f0.x;
    let dx = f32(sx) + 0.5 - hw;
    let dy = f32(sy) + 0.5 - hh;
    let ro = sqrt((dx * dx + dy * dy) / hd2);
    var rgb = vec3<u32>(0u);
    for (var ch = 0u; ch < 3u; ch = ch + 1u) {
        let t = ro / (1.0 + ca * (1.0 - f32(ch)));
        var xr = t;
        for (var i = 0; i < 3; i = i + 1) {
            let fx = xr * (1.0 + kc * xr * xr) - t;
            xr = xr - fx / (1.0 + 3.0 * kc * xr * xr);
        }
        var sc = 1.0;
        if (ro > 0.0) {
            sc = xr / ro;
        }
        rgb[ch] = u32(clamp(floor(rgb_at(hw + dx * sc, hh + dy * sc, ch) + 0.5), 0.0, 255.0));
    }
    return vec4<u32>(rgb.x, rgb.y, rgb.z, src_alpha(p));
}
