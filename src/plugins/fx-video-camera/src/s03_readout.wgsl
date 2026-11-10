// P1b the readout (cpu.rs readout_row): blooming (the charge above the well spills into the four
// neighbours), the well clips, row noise (temporal) and the column fixed pattern, read noise, the
// analog gain and the ADC → DN, 16-bit.
// u0 = (seed_t, seed_f, adc_max | black << 16, flags); f0 = (fw, cg, read, bloom);
// f1 = (row e-, column e-, 0, 0). T0 = P1a.

fn n_at(sx: i32, sy: i32) -> f32 {
    return f32(v16(at(sx, sy))) * 6.1035156e-5 * P.f0.x;
}

fn spill(q: f32) -> f32 {
    return max(q - P.f0.x, 0.0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    begin(P.u0.w);
    let p = vec2<i32>(floor(pos.xy));
    let k = KO();
    let sx = p.x / k;
    let sy = p.y / k;
    let fw = P.f0.x;
    let seed_t = P.u0.x;
    let seed_f = P.u0.y;
    let adc_max = P.u0.z & 0xFFFFu;
    let black = f32(P.u0.z >> 16u);
    var row = 0.0;
    if (P.f1.x > 0.0) {
        row = P.f1.x * gauss(hash2(0u, u32(sy), seed_t ^ 0x77777777u));
    }
    var n = n_at(sx, sy);
    if (P.f0.w > 0.0) {
        let s = spill(n_at(sx - 1, sy)) + spill(n_at(sx + 1, sy)) + spill(n_at(sx, sy - 1)) + spill(n_at(sx, sy + 1));
        n = n + s * (P.f0.w * 0.25);
    }
    n = min(n, fw);
    n = n + row;
    if (P.f1.y > 0.0) {
        n = n + P.f1.y * gauss(hash2(u32(sx), 0u, seed_f ^ 0x33333333u));
    }
    if (((FL >> 13u) & 1u) == 1u) {
        n = n + P.f0.z * gauss(hash2(u32(sx), u32(sy), seed_t ^ 0x55555555u));
    }
    let dn = clamp(floor(black + n * P.f0.y + 0.5), 0.0, f32(adc_max));
    return t16(u32(dn));
}
