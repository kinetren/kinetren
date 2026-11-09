// P2a the raw front (cpu.rs raw_front_row): black-level subtraction, the optical-black row/column
// correction, defect-pixel correction, green equalisation → normalised raw, 16-bit fixed.
// u0 = the ISP's table; u1 = (flags, seed_t, seed_f, 0); f0 = (b_isp, inv_range, row_corr,
// col_corr); f1 = (dpc_thr, geq, 0, 0). T0 = P1b.

// the optical-black corrections are one per row and one per column: a window takes its five rows'
// and five columns once (gpu-pass-cost; the 5×5 defect search had evaluated them for every
// neighbour, up to 50 hashes a pixel)
fn row_corr(sy: i32) -> f32 {
    if (P.f0.z != 0.0) {
        return P.f0.z * gauss(hash2(0u, u32(sy), P.u1.y ^ 0x77777777u));
    }
    return 0.0;
}

fn col_corr(sx: i32) -> f32 {
    if (P.f0.w != 0.0) {
        return P.f0.w * gauss(hash2(u32(sx), 0u, P.u1.z ^ 0x33333333u));
    }
    return 0.0;
}

// the black-corrected value at (sx, sy) with that row's and column's corrections
fn vb(sx: i32, sy: i32, rc: f32, cc: f32) -> f32 {
    var v = (f32(v16(at(sx, sy))) - P.f0.x) * P.f0.y;
    if (P.f0.z != 0.0) {
        v = v - rc;
    }
    if (P.f0.w != 0.0) {
        v = v - cc;
    }
    return v;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    begin(P.u1.x);
    TAB = P.u0;
    let p = vec2<i32>(floor(pos.xy));
    let k = KO();
    let sx = p.x / k;
    let sy = p.y / k;
    let s = sdims();
    // the window's columns and rows: wrapped positions, their corrections, their mosaic phases
    var wx: array<i32, 5>;
    var wy: array<i32, 5>;
    var cc: array<f32, 5>;
    var rc: array<f32, 5>;
    var cm: array<i32, 5>;
    var rm: array<i32, 5>;
    for (var i = 0; i < 5; i = i + 1) {
        wx[i] = wrap1(sx + i - 2, s.x);
        wy[i] = wrap1(sy + i - 2, s.y);
        cc[i] = col_corr(wx[i]);
        rc[i] = row_corr(wy[i]);
        cm[i] = pmod(sx + i - 2);
        rm[i] = pmod(sy + i - 2);
    }
    let code = cfa_mod(cm[2], rm[2]);
    var v = vb(sx, sy, rc[2], cc[2]);
    if (((FL >> 14u) & 1u) == 1u) {
        let thr = P.f1.x;
        var mn = 3.4028235e38;
        var mx = -3.4028235e38;
        var found = false;
        for (var j = 0; j < 5; j = j + 1) {
            for (var i = 0; i < 5; i = i + 1) {
                if ((i == 2 && j == 2) || cfa_mod(cm[i], rm[j]) != code) {
                    continue;
                }
                let q = vb(wx[i], wy[j], rc[j], cc[i]);
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
        for (var c = 0; c < 4; c = c + 1) {
            let i = select(1, 3, (c & 1) == 1);
            let j = select(1, 3, c >= 2);
            if (cfa_mod(cm[i], rm[j]) == 1u) {
                sum = sum + vb(wx[i], wy[j], rc[j], cc[i]);
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
