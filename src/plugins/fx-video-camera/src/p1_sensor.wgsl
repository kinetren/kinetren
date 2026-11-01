// P1 the sensor's electronics (cpu.rs p1_row): PRNU, DSNU, hot/dead pixels (the sensor seed), shot
// and read noise (the frame seed), the analog gain and the ADC → DN, 16-bit in R,G.
// u0 = (seed_t, seed_f, adc_max | black << 16, dead_thr); u1 = (hot_thr, flags, 0, 0);
// f0 = (fw, cg, read, dark); f1 = (dsnu, prnu, 0, 0). T0 = P0.

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
    FL = P.u1.y;
    let p = vec2<i32>(floor(pos.xy));
    let k = u32(KK());
    let sx = u32(p.x) / k;
    let sy = u32(p.y) / k;
    let fw = P.f0.x;
    let lam_s = f32(v16(textureLoad(T0, p, 0))) * 6.1035156e-5 * fw;
    let seed_t = P.u0.x;
    let seed_f = P.u0.y;
    let adc_max = P.u0.z & 0xFFFFu;
    let black = f32(P.u0.z >> 16u);
    let hf = hash2(sx, sy, seed_f);
    var dn = black;
    if (hf >= P.u0.w) {
        let gpx = 1.0 + P.f1.y * gauss(hash2(sx, sy, seed_f ^ 0x11111111u));
        var dk = max(P.f0.w + P.f1.x * gauss(hash2(sx, sy, seed_f ^ 0x22222222u)), 0.0);
        if (hf - P.u0.w < P.u1.x) {
            dk = dk + fw * (0.2 + 0.8 * unit(nh(hf)));
        }
        let lam = lam_s * gpx + dk;
        var n = 0.0;
        if (((FL >> 13u) & 1u) == 1u) {
            n = min(poisson(lam, hash2(sx, sy, seed_t)), fw);
            n = n + P.f0.z * gauss(hash2(sx, sy, seed_t ^ 0x55555555u));
        } else {
            n = min(lam, fw);
        }
        dn = clamp(floor(black + n * P.f0.y + 0.5), 0.0, f32(adc_max));
    }
    let q = u32(dn);
    return vec4<u32>(q >> 8u, q & 255u, 0u, 255u);
}
