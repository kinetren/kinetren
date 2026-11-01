// P2a the raw front (cpu.rs raw_front_row): black-level subtraction, the optical-black row/column
// correction, defect-pixel correction, green equalisation → normalised raw, 16-bit fixed.
// u0 = the ISP's table; u1 = (flags, seed_t, seed_f, 0); f0 = (b_isp, inv_range, row_corr,
// col_corr); f1 = (dpc_thr, geq, 0, 0). T0 = P1b.

fn vb(sx: i32, sy: i32) -> f32 {
    var v = (f32(v16(at(sx, sy))) - P.f0.x) * P.f0.y;
    if (P.f0.z != 0.0) {
        v = v - P.f0.z * gauss(hash2(0u, u32(sy), P.u1.y ^ 0x77777777u));
    }
    if (P.f0.w != 0.0) {
        v = v - P.f0.w * gauss(hash2(u32(sx), 0u, P.u1.z ^ 0x33333333u));
    }
    return v;
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
        let thr = P.f1.x;
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
    let geq = P.f1.y;
    if (geq > 0.0 && code == 1u) {
        var sum = 0.0;
        var n = 0.0;
        for (var i = 0; i < 4; i = i + 1) {
            let dx = select(-1, 1, (i & 1) == 1);
            let dy = select(-1, 1, i >= 2);
            if (cfa_code(sx + dx, sy + dy) == 1u) {
                sum = sum + vb(wrap1(sx + dx, s.x), wrap1(sy + dy, s.y));
                n = n + 1.0;
            }
        }
        if (n > 0.0) {
            let d = sum / n - v;
            if (abs(d) < 0.05) {
                v = v + geq * 0.5 * d;
            }
        }
    }
    return t16(fix16(max(v, 0.0)));
}
