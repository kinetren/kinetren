// P2 the ISP's raw front (cpu.rs p2_row): black-level subtraction, defect-pixel correction, RGB-IR
// subtraction, lens shading, white balance and digital gain → linear, 16-bit fixed (×16384).
// u0 = the ISP's (phase-shifted) table; u1 = (flags, bits(dpc_thr), bits(ir_sub), bits(lsc_pow));
// f0 = (b_isp, inv_range, vig_t2, 0); f1 = gains (R, G, B, W). T0 = P1.

fn vb(sx: i32, sy: i32) -> f32 {
    return (f32(v16(at(sx, sy))) - P.f0.x) * P.f0.y;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    FL = P.u1.x;
    TAB = P.u0;
    let p = vec2<i32>(floor(pos.xy));
    let k = KK();
    let sx = p.x / k;
    let sy = p.y / k;
    let s = sdims();
    let code = cfa_code(sx, sy);
    var v = vb(sx, sy);
    if (((FL >> 14u) & 1u) == 1u) {
        let thr = bitcast<f32>(P.u1.y);
        var mn = 3.4028235e38;
        var mx = -3.4028235e38;
        var found = false;
        for (var dy = -2; dy <= 2; dy = dy + 1) {
            for (var dx = -2; dx <= 2; dx = dx + 1) {
                if ((dx == 0 && dy == 0) || cfa_code(sx + dx, sy + dy) != code) {
                    continue;
                }
                let q = vb(wrap1(sx + dx, s.x), wrap1(sy + dy, s.y));
                mn = min(mn, q);
                mx = max(mx, q);
                found = true;
            }
        }
        if (found) {
            if (v > mx + thr) {
                v = mx;
            } else if (v < mn - thr) {
                v = mn;
            }
        }
    }
    let irs = bitcast<f32>(P.u1.z);
    if (irs > 0.0 && code != 4u) {
        var sum = 0.0;
        var n = 0.0;
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                if (cfa_code(sx + dx, sy + dy) == 4u) {
                    sum = sum + vb(wrap1(sx + dx, s.x), wrap1(sy + dy, s.y));
                    n = n + 1.0;
                }
            }
        }
        if (n > 0.0) {
            v = v - irs * (dye_nir(code) / dye_nir(4u)) * (sum / n);
        }
    }
    let lp = bitcast<f32>(P.u1.w);
    if (lp > 0.0) {
        v = v * pow(1.0 + P.f0.z * r2_of(sx, sy), lp);
    }
    if (code < 4u) {
        v = v * P.f1[code];
    }
    let q = fix16(max(v, 0.0));
    return vec4<u32>(q >> 8u, q & 255u, 0u, 255u);
}
