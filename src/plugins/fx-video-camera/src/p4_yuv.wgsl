// P4 YUV, denoise and sharpening (cpu.rs p4_row) → Y,U,V 8-bit (or RGB8 when YUV is bypassed).
// u0 = (flags, bits(dec Kr), bits(dec Kb), bits(1/Kg)); u1 = (bits(2(1−Kr)), bits(2(1−Kb)), 0, 0);
// f0 = (Kr, Kg, Kb, 0.5/(1−Kb)); f1 = (0.5/(1−Kr), luma 1/σ², chroma amount, sharpen).
// T0 = P3, T1 = the source.

fn yuv(sx: i32, sy: i32) -> vec3<f32> {
    let t = at(sx, sy);
    let r = f32(t.r) * 0.003921569;
    let g = f32(t.g) * 0.003921569;
    let b = f32(t.b) * 0.003921569;
    let yy = P.f0.x * r + P.f0.y * g + P.f0.z * b;
    return vec3<f32>(yy, (b - yy) * P.f0.w, (r - yy) * P.f1.x);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    FL = P.u0.x;
    let p = vec2<i32>(floor(pos.xy));
    let k = KK();
    let sx = p.x / k;
    let sy = p.y / k;
    let c0 = yuv(sx, sy);
    let y0 = c0.x;
    var u = c0.y;
    var v = c0.z;
    var ynr = y0;
    let inv_s2 = P.f1.y;
    if (inv_s2 > 0.0) {
        var s = 0.0;
        var sw = 0.0;
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                let q = yuv(sx + dx, sy + dy).x;
                let d = q - y0;
                let wt = 1.0 / (1.0 + d * d * inv_s2);
                s = s + wt * q;
                sw = sw + wt;
            }
        }
        ynr = s / sw;
    }
    let amt = P.f1.z;
    if (amt > 0.0) {
        var su = 0.0;
        var sv = 0.0;
        var sw = 0.0;
        for (var dy = -2; dy <= 2; dy = dy + 1) {
            for (var dx = -2; dx <= 2; dx = dx + 1) {
                let q = yuv(sx + dx, sy + dy);
                let d = q.x - y0;
                let wt = 1.0 / (1.0 + d * d * 277.77778);
                su = su + wt * q.y;
                sv = sv + wt * q.z;
                sw = sw + wt;
            }
        }
        u = u + amt * (su / sw - u);
        v = v + amt * (sv / sw - v);
    }
    var yo = ynr;
    if (P.f1.w > 0.0) {
        var s = 0.0;
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                s = s + yuv(sx + dx, sy + dy).x;
            }
        }
        yo = ynr + P.f1.w * (y0 - s * 0.11111111);
    }
    let a = src_alpha(p);
    if (((FL >> 16u) & 1u) == 1u) {
        return vec4<u32>(byte(yo), byte(u + 0.5), byte(v + 0.5), a);
    }
    let r = yo + bitcast<f32>(P.u1.x) * v;
    let b = yo + bitcast<f32>(P.u1.y) * u;
    let g = (yo - bitcast<f32>(P.u0.y) * r - bitcast<f32>(P.u0.z) * b) * bitcast<f32>(P.u0.w);
    return vec4<u32>(byte(r), byte(g), byte(b), a);
}
