// P2n raw NR by the sensor's noise model (cpu.rs raw_nr_row): a same-colour 5×5 range-weighted mean
// whose range is the noise the model predicts at the pixel's level, σ² = a·v + b.
// u0 = the ISP's table; u1 = (flags, 0, 0, 0); f0 = (a, b, strength, 0). T0 = P2a.

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    begin(P.u1.x);
    TAB = P.u0;
    let p = vec2<i32>(floor(pos.xy));
    let k = KO();
    let sx = p.x / k;
    let sy = p.y / k;
    // the window's wrapped columns and rows and their mosaic phases, once (gpu-pass-cost)
    let sd = sdims();
    var wx: array<i32, 5>;
    var wy: array<i32, 5>;
    var cm: array<i32, 5>;
    var rm: array<i32, 5>;
    for (var i = 0; i < 5; i = i + 1) {
        wx[i] = wrap1(sx + i - 2, sd.x);
        wy[i] = wrap1(sy + i - 2, sd.y);
        cm[i] = pmod(sx + i - 2);
        rm[i] = pmod(sy + i - 2);
    }
    let code = cfa_mod(cm[2], rm[2]);
    let v0 = f32(v16(at(wx[2], wy[2]))) * 6.1035156e-5;
    let k2 = 4.0 * P.f0.z * P.f0.z;
    let h2 = k2 * (P.f0.x * max(v0, 0.0) + P.f0.y) + 1.0e-12;
    var s = 0.0;
    var sw = 0.0;
    for (var j = 0; j < 5; j = j + 1) {
        for (var i = 0; i < 5; i = i + 1) {
            if (cfa_mod(cm[i], rm[j]) != code) {
                continue;
            }
            let q = f32(v16(at(wx[i], wy[j]))) * 6.1035156e-5;
            let d = q - v0;
            let wt = 1.0 / (1.0 + d * d / h2);
            s = s + wt * q;
            sw = sw + wt;
        }
    }
    return t16(fix16(s / sw));
}
