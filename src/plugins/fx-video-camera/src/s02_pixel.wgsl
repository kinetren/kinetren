// P1a the pixel (cpu.rs pixel_row): PRNU, Gr/Gb imbalance, dark current with DSNU, hot and dead
// pixels (the sensor seed), shot noise (the frame seed) → the charge as a fraction of the well,
// 16-bit fixed, NOT yet clipped (the readout spills the excess).
// u0 = sensor table; u1 = (flags, seed_t, seed_f, 0); f0 = (fw, dark, dsnu, prnu);
// f1 = (dead_thr, hot_thr, gr/gb imbalance, 1/fw). T0 = P0.

fn poisson(lambda: f32, h1: u32) -> f32 {
    if (lambda < 12.0) {
        let u = unit(h1);
        var p = exp(-lambda);
        var c = p;
        var k = 0.0;
        loop {
            if (!(u > c && k < 64.0)) {
                break;
            }
            k = k + 1.0;
            p = p * lambda / k;
            c = c + p;
        }
        return k;
    }
    return max(floor(lambda + sqrt(lambda) * gauss(h1) + 0.5), 0.0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    FL = P.u1.x;
    TAB = P.u0;
    let p = vec2<i32>(floor(pos.xy));
    let k = u32(KK());
    let sx = u32(p.x) / k;
    let sy = u32(p.y) / k;
    let fw = P.f0.x;
    let lam_s = f32(v16(textureLoad(T0, p, 0))) * 6.1035156e-5 * fw;
    let seed_t = P.u1.y;
    let seed_f = P.u1.z;
    let dead_thr = u32(P.f1.x);
    let hot_thr = u32(P.f1.y);
    let hf = hash2(sx, sy, seed_f);
    var n = 0.0;
    if (hf >= dead_thr) {
        var gpx = 1.0 + P.f0.w * gauss(hash2(sx, sy, seed_f ^ 0x11111111u));
        let code = cfa_code(i32(sx), i32(sy));
        let gimb = P.f1.z;
        if (code == 1u && gimb > 0.0) {
            let side = cfa_code(i32(sx) + 1, i32(sy));
            if (side == 0u) {
                gpx = gpx * (1.0 + gimb);
            } else if (side == 2u) {
                gpx = gpx * (1.0 - gimb);
            }
        }
        var dk = max(P.f0.y + P.f0.z * gauss(hash2(sx, sy, seed_f ^ 0x22222222u)), 0.0);
        if (hf - dead_thr < hot_thr) {
            dk = dk + fw * (0.2 + 0.8 * unit(nh(hf)));
        }
        let lam = lam_s * gpx + dk;
        if (((FL >> 13u) & 1u) == 1u) {
            n = poisson(lam, hash2(sx, sy, seed_t));
        } else {
            n = lam;
        }
    }
    return t16(fix16(n * P.f1.w));
}
