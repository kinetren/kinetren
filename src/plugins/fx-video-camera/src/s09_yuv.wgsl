// P4 YUV, denoise, false-colour and purple-fringe suppression, sharpening (cpu.rs yuv_row) → Y,U,V
// 8-bit (or RGB8 when YUV is bypassed).
// u0 = (flags, bits(dec Kr), bits(dec Kb), bits(1/Kg)); u1 = (bits(2(1−Kr)), bits(2(1−Kb)), bits(fcs), bits(purple));
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
    begin(P.u0.x);
    let p = vec2<i32>(floor(pos.xy));
    let k = KK();
    let sx = p.x / k;
    let sy = p.y / k;
    // the 5×5 neighbourhood's YUV once (gpu-pass-cost): the five filters below read it up to 72
    // times a pixel; the 3×3 alone when no 5×5 filter runs
    let wide = P.f1.z > 0.0 || bitcast<f32>(P.u1.w) > 0.0;
    var nb: array<vec3<f32>, 25>;
    for (var j = 0; j < 5; j = j + 1) {
        for (var i = 0; i < 5; i = i + 1) {
            if (wide || (i >= 1 && i <= 3 && j >= 1 && j <= 3)) {
                nb[j * 5 + i] = yuv(sx + i - 2, sy + j - 2);
            }
        }
    }
    let c0 = nb[12];
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
                let q = nb[(dy + 2) * 5 + dx + 2].x;
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
                let q = nb[(dy + 2) * 5 + dx + 2];
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
    let fcs = bitcast<f32>(P.u1.z);
    if (fcs > 0.0) {
        let gx = abs(nb[13].x - nb[11].x);
        let gy = abs(nb[17].x - nb[7].x);
        let kk = 1.0 - fcs * clamp(max(gx, gy) * 4.0, 0.0, 1.0);
        u = u * kk;
        v = v * kk;
    }
    let purple = bitcast<f32>(P.u1.w);
    if (purple > 0.0 && u > 0.0 && v > 0.0) {
        var my = 0.0;
        for (var dy = -2; dy <= 2; dy = dy + 1) {
            for (var dx = -2; dx <= 2; dx = dx + 1) {
                my = max(my, nb[(dy + 2) * 5 + dx + 2].x);
            }
        }
        let kk = 1.0 - purple * clamp((my - 0.9) * 10.0, 0.0, 1.0) * clamp(min(u, v) * 8.0, 0.0, 1.0);
        u = u * kk;
        v = v * kk;
    }
    var yo = ynr;
    if (P.f1.w > 0.0) {
        var s = 0.0;
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                s = s + nb[(dy + 2) * 5 + dx + 2].x;
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
