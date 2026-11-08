// P3 demosaic under the ISP's assumed layout, highlight handling, the CCM and the tone curve
// (cpu.rs demosaic_row) → RGB8 and the source's alpha. u0 = the ISP's table;
// u1 = (flags, bits(ccm[8]), bits(highlight), bits(clip)); f0, f1 = ccm[0..8]. T0 = P2b, T1 = the source.

// (sum, count) of the neighbours of code `want` within r, self excluded
fn near(sx: i32, sy: i32, want: u32, r: i32) -> vec2<f32> {
    var acc = vec2<f32>(0.0);
    for (var dy = -r; dy <= r; dy = dy + 1) {
        for (var dx = -r; dx <= r; dx = dx + 1) {
            if ((dx != 0 || dy != 0) && cfa_code(sx + dx, sy + dy) == want) {
                acc = vec2<f32>(acc.x + vfix(sx + dx, sy + dy), acc.y + 1.0);
            }
        }
    }
    return acc;
}

fn bilinear(sx: i32, sy: i32, want: u32) -> f32 {
    var a = near(sx, sy, want, 1);
    if (a.y == 0.0) {
        a = near(sx, sy, want, 2);
    }
    if (a.y > 0.0) {
        return a.x / a.y;
    }
    return 0.0;
}

fn gest(sx: i32, sy: i32) -> f32 {
    let v0 = vfix(sx, sy);
    if (cfa_code(sx, sy) == 1u) {
        return v0;
    }
    for (var r = 1; r <= 2; r = r + 1) {
        var s = 0.0;
        var sw = 0.0;
        for (var dy = -r; dy <= r; dy = dy + 1) {
            for (var dx = -r; dx <= r; dx = dx + 1) {
                if ((dx == 0 && dy == 0) || cfa_code(sx + dx, sy + dy) != 1u) {
                    continue;
                }
                let ux = isign(dx);
                let uy = isign(dy);
                let g1 = abs(vfix(sx + ux, sy + uy) - vfix(sx - ux, sy - uy));
                let g2 = abs(2.0 * v0 - vfix(sx + 2 * ux, sy + 2 * uy) - vfix(sx - 2 * ux, sy - 2 * uy));
                let wt = 1.0 / (0.00390625 + g1 + g2);
                s = s + wt * vfix(sx + dx, sy + dy);
                sw = sw + wt;
            }
        }
        if (sw > 0.0) {
            return s / sw;
        }
    }
    return v0;
}

fn edge(sx: i32, sy: i32, want: u32) -> f32 {
    let gp = gest(sx, sy);
    if (want == 1u) {
        return gp;
    }
    for (var r = 1; r <= 2; r = r + 1) {
        var s = 0.0;
        var n = 0.0;
        for (var dy = -r; dy <= r; dy = dy + 1) {
            for (var dx = -r; dx <= r; dx = dx + 1) {
                if ((dx != 0 || dy != 0) && cfa_code(sx + dx, sy + dy) == want) {
                    s = s + (vfix(sx + dx, sy + dy) - gest(sx + dx, sy + dy));
                    n = n + 1.0;
                }
            }
        }
        if (n > 0.0) {
            return gp + s / n;
        }
    }
    return gp;
}

fn directional(sx: i32, sy: i32, want: u32, v0: f32) -> f32 {
    for (var r = 1; r <= 2; r = r + 1) {
        var s = 0.0;
        var sw = 0.0;
        for (var dy = -r; dy <= r; dy = dy + 1) {
            for (var dx = -r; dx <= r; dx = dx + 1) {
                if ((dx == 0 && dy == 0) || cfa_code(sx + dx, sy + dy) != want) {
                    continue;
                }
                let ux = isign(dx);
                let uy = isign(dy);
                let g1 = abs(vfix(sx + ux, sy + uy) - vfix(sx - ux, sy - uy));
                let g2 = abs(2.0 * v0 - vfix(sx + 2 * ux, sy + 2 * uy) - vfix(sx - 2 * ux, sy - 2 * uy));
                let wt = 1.0 / (0.00390625 + g1 + g2);
                s = s + wt * vfix(sx + dx, sy + dy);
                sw = sw + wt;
            }
        }
        if (sw > 0.0) {
            return s / sw;
        }
    }
    return 0.0;
}

fn mhc_bayer(sx: i32, sy: i32, code: u32, want: u32) -> f32 {
    let v0 = vfix(sx, sy);
    let axial2 = vfix(sx - 2, sy) + vfix(sx + 2, sy) + vfix(sx, sy - 2) + vfix(sx, sy + 2);
    let diag = vfix(sx - 1, sy - 1) + vfix(sx + 1, sy - 1) + vfix(sx - 1, sy + 1) + vfix(sx + 1, sy + 1);
    if (want == 1u) {
        return (4.0 * v0 + 2.0 * (vfix(sx - 1, sy) + vfix(sx + 1, sy) + vfix(sx, sy - 1) + vfix(sx, sy + 1)) - axial2) * 0.125;
    }
    if (code == 1u) {
        var a1 = 0.0;
        var a2 = 0.0;
        var b2 = 0.0;
        if (cfa_code(sx + 1, sy) == want) {
            a1 = vfix(sx - 1, sy) + vfix(sx + 1, sy);
            a2 = vfix(sx - 2, sy) + vfix(sx + 2, sy);
            b2 = vfix(sx, sy - 2) + vfix(sx, sy + 2);
        } else {
            a1 = vfix(sx, sy - 1) + vfix(sx, sy + 1);
            a2 = vfix(sx, sy - 2) + vfix(sx, sy + 2);
            b2 = vfix(sx - 2, sy) + vfix(sx + 2, sy);
        }
        return (5.0 * v0 + 4.0 * a1 - a2 - diag + 0.5 * b2) * 0.125;
    }
    return (6.0 * v0 + 2.0 * diag - 1.5 * axial2) * 0.125;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    begin(P.u1.x);
    TAB = P.u0;
    let p = vec2<i32>(floor(pos.xy));
    let k = KK();
    let sx = p.x / k;
    let sy = p.y / k;
    let code = cfa_code(sx, sy);
    let v0 = vfix(sx, sy);
    let method = (FL >> 20u) & 15u;
    let bayer = ((FL >> 15u) & 1u) == 1u;
    var rgb = vec3<f32>(0.0);
    for (var want = 0u; want < 3u; want = want + 1u) {
        var e = 0.0;
        if (want == code) {
            e = v0;
        } else if (method == 0u) {
            e = bilinear(sx, sy, want);
        } else if (method == 1u) {
            if (bayer) {
                e = mhc_bayer(sx, sy, code, want);
            } else if (code == 4u) {
                e = bilinear(sx, sy, want);
            } else {
                let a = near(sx, sy, code, 2);
                var own = v0;
                if (a.y > 0.0) {
                    own = a.x / a.y;
                }
                e = bilinear(sx, sy, want) + 0.5 * (v0 - own);
            }
        } else if (bayer) {
            e = edge(sx, sy, want);
        } else {
            e = directional(sx, sy, want, v0);
        }
        rgb[want] = e;
    }
    let hl = bitcast<f32>(P.u1.z);
    if (hl > 0.0) {
        let clip = bitcast<f32>(P.u1.w);
        let m = max(max(rgb.x, rgb.y), rgb.z);
        let t = clamp((m - 0.9 * clip) / (0.1 * clip), 0.0, 1.0) * hl;
        let yy = 0.2126 * rgb.x + 0.7152 * rgb.y + 0.0722 * rgb.z;
        rgb = vec3<f32>(rgb.x + (yy - rgb.x) * t, rgb.y + (yy - rgb.y) * t, rgb.z + (yy - rgb.z) * t);
    }
    let m8 = bitcast<f32>(P.u1.y);
    let r = P.f0.x * rgb.x + P.f0.y * rgb.y + P.f0.z * rgb.z;
    let g = P.f0.w * rgb.x + P.f1.x * rgb.y + P.f1.y * rgb.z;
    let b = P.f1.z * rgb.x + P.f1.w * rgb.y + m8 * rgb.z;
    let tone = (FL >> 24u) & 15u;
    return vec4<u32>(byte(oetf(r, tone)), byte(oetf(g, tone)), byte(oetf(b, tone)), src_alpha(p));
}
