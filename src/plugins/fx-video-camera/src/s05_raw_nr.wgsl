// P2n raw NR by the sensor's noise model (cpu.rs raw_nr_row): a same-colour 5×5 range-weighted mean
// whose range is the noise the model predicts at the pixel's level, σ² = a·v + b.
// u0 = the ISP's table; u1 = (flags, 0, 0, 0); f0 = (a, b, strength, 0). T0 = P2a.

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    FL = P.u1.x;
    TAB = P.u0;
    let p = vec2<i32>(floor(pos.xy));
    let k = KK();
    let sx = p.x / k;
    let sy = p.y / k;
    let code = cfa_code(sx, sy);
    let v0 = vfix(sx, sy);
    let k2 = 4.0 * P.f0.z * P.f0.z;
    let h2 = k2 * (P.f0.x * max(v0, 0.0) + P.f0.y) + 1.0e-12;
    var s = 0.0;
    var sw = 0.0;
    for (var dy = -2; dy <= 2; dy = dy + 1) {
        for (var dx = -2; dx <= 2; dx = dx + 1) {
            if (cfa_code(sx + dx, sy + dy) != code) {
                continue;
            }
            let q = vfix(sx + dx, sy + dy);
            let d = q - v0;
            let wt = 1.0 / (1.0 + d * d / h2);
            s = s + wt * q;
            sw = sw + wt;
        }
    }
    return t16(fix16(s / sw));
}
