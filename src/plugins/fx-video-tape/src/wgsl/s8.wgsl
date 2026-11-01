// vhs_tape stage 8 (`sim::s8`): the colour-under demodulated with the track's rotation undone, the
// V switch undone, ACC. T0 = the RF, T1 = the line table. u0 = (std, W); f0 = (fcu_n, cu_line).

fn kch(i: i32, s: u32) -> f32 {
    if (s == 0u) { return K_CHROMA_P[i]; }
    return K_CHROMA_N[i];
}

fn cu_phase(j: i32, yf: f32, rot: f32) -> f32 {
    let l = P.f0.y * yf + rot * yf * 0.25;
    return P.f0.x * f32(j) + (l - floor(l));
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let n = i32(floor(pos.x));
    let y = i32(floor(pos.y));
    let s = P.u0.x;
    let yf = ld1f(12, y);
    let rot = ld1f(10, y);
    var len = K_CHROMA_P_LEN;
    if (s != 0u) { len = K_CHROMA_N_LEN; }
    var i0 = 0.0;
    var q0 = 0.0;
    for (var i = 0; i < len; i = i + 1) {
        let j = n + K_CHROMA_C - i;
        let th = cu_phase(j, yf, rot);
        let x = ld0f(j, y);
        let c = kch(i, s);
        i0 = i0 + c * (x * cos_cyc(th));
        q0 = q0 + c * (x * sin_cyc(th));
    }
    let acc = ld1f(9, y);
    let uu = 2.0 * i0 * acc;
    let vv = 2.0 * q0 * acc * ld1f(11, y);
    return put2(uu, 2.0, 16384.0, vv, 2.0, 16384.0);
}
