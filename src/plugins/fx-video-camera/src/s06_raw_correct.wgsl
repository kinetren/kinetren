// P2b (cpu.rs raw_correct_row): RGB-IR subtraction, lens shading (luma), colour lens shading, white
// balance and digital gain → normalised linear raw, 16-bit fixed.
// u0 = the ISP's table; u1 = (flags, bits(ir_sub), bits(lsc_pow), bits(colour lsc));
// f0 = (vig_t2, colour shading, 0, 0); f1 = gains (R, G, B, W). T0 = P2a or P2n.

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    begin(P.u1.x);
    TAB = P.u0;
    let p = vec2<i32>(floor(pos.xy));
    let k = KK();
    let sx = p.x / k;
    let sy = p.y / k;
    let code = cfa_code(sx, sy);
    var v = vfix(sx, sy);
    let irs = bitcast<f32>(P.u1.y);
    if (irs > 0.0 && code != 4u) {
        var sum = 0.0;
        var n = 0.0;
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                if (cfa_code(sx + dx, sy + dy) == 4u) {
                    sum = sum + vfix(sx + dx, sy + dy);
                    n = n + 1.0;
                }
            }
        }
        if (n > 0.0) {
            v = v - irs * (dye_nir(code) / dye_nir(4u)) * (sum / n);
        }
    }
    let r2 = r2_of(sx, sy);
    let lp = bitcast<f32>(P.u1.z);
    if (lp > 0.0) {
        v = v * pow(1.0 + P.f0.x * r2, lp);
    }
    let cl = bitcast<f32>(P.u1.w);
    if (cl > 0.0) {
        if (code == 0u) {
            v = v * pow(max(1.0 - P.f0.y * r2, 0.05), -cl);
        } else if (code == 2u) {
            v = v * pow(max(1.0 + 0.5 * P.f0.y * r2, 0.05), -cl);
        }
    }
    if (code < 4u) {
        v = v * P.f1[code];
    }
    return t16(fix16(max(v, 0.0)));
}
