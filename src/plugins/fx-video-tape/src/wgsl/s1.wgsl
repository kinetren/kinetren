// vhs_tape stage 1 (`sim::s1`): the record luma → LPF ∗ pre-emphasis → clip → the DDS increment.
// T0 = the picture resampled to A×H. u0 = (-, W, M, A); f0 = (dark, white, fsync_n, dev_n),
// f1.x = l_sync.

fn luma_at(j: i32, y: i32) -> f32 {
    let x = j - i32(P.u0.z);
    if (x < 0 || x >= i32(P.u0.w)) { return 0.0; }
    let t = textureLoad(T0, vec2<i32>(x, y), 0);
    return (0.299 * f32(t.r) + 0.587 * f32(t.g) + 0.114 * f32(t.b)) * (1.0 / 255.0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let n = i32(floor(pos.x));
    let y = i32(floor(pos.y));
    var e = 0.0;
    // @TAPS REC e = e + {c} * luma_at(n + K_REC_C - {i}, y);
    e = clamp(e, P.f0.x, P.f0.y);
    let f = P.f0.z + (e - P.f1.x) * P.f0.w;
    return put(u32(f * 4294967296.0));
}
