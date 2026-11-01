// vhs_tape stage 7 (`sim::s7`): the dropout compensator (the line before in the field), then the
// playback LPF ∗ de-emphasis. T0 = stage 6 (level | envelope). u0 = (std, W, doc); f0.x = threshold.

fn kpl(i: i32, s: u32) -> f32 {
    if (s == 0u) { return K_PLAY_P[i]; }
    return K_PLAY_N[i];
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let n = i32(floor(pos.x));
    let y = i32(floor(pos.y));
    let s = P.u0.x;
    var len = K_PLAY_P_LEN;
    if (s != 0u) { len = K_PLAY_N_LEN; }
    var acc = 0.0;
    for (var i = 0; i < len; i = i + 1) {
        let j = n + K_PLAY_C - i;
        let t = ld0(j, y);
        var v = lo16(t, 2.0, 8192.0);
        if (P.u0.z != 0u && y >= 2 && hi16(t, 0.0, 16384.0) < P.f0.x) {
            v = lo16(ld0(j, y - 2), 2.0, 8192.0);
        }
        acc = acc + kpl(i, s) * v;
    }
    return putf(acc);
}
