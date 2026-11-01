// P5 chroma subsampling and the decoder (cpu.rs chroma_row) → RGB8 and the source's alpha.
// u0 = (flags, 0, 0, 0); f0 = (Kr, Kb, 1/Kg, 2(1−Kr)); f1 = (2(1−Kb), 0, 0, 0). T0 = P4, T1 = the source.

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    FL = P.u0.x;
    let p = vec2<i32>(floor(pos.xy));
    let k = KK();
    let sx = p.x / k;
    let sy = p.y / k;
    let s = sdims();
    let t = at(sx, sy);
    let mode = (FL >> 28u) & 3u;
    var bx = sx;
    var by = sy;
    var bw = 1;
    var bh = 1;
    if (mode == 1u) {
        bx = sx & ~1;
        bw = 2;
    } else if (mode == 2u) {
        bx = sx & ~1;
        by = sy & ~1;
        bw = 2;
        bh = 2;
    }
    var su = 0u;
    var sv = 0u;
    var n = 0u;
    for (var dy = 0; dy < bh; dy = dy + 1) {
        for (var dx = 0; dx < bw; dx = dx + 1) {
            if (bx + dx < s.x && by + dy < s.y) {
                let q = at(bx + dx, by + dy);
                su = su + q.g;
                sv = sv + q.b;
                n = n + 1u;
            }
        }
    }
    let u8v = (su + n / 2u) / n;
    let v8v = (sv + n / 2u) / n;
    let yy = f32(t.r) * 0.003921569;
    let u = f32(u8v) * 0.003921569 - 0.5;
    let v = f32(v8v) * 0.003921569 - 0.5;
    let r = yy + P.f0.w * v;
    let b = yy + P.f1.x * u;
    let g = (yy - P.f0.x * r - P.f0.y * b) * P.f0.z;
    return vec4<u32>(byte(r), byte(g), byte(b), src_alpha(p));
}
