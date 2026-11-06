// vhs_tape stage 8 (`sim::s8`): the colour-under demodulated with the track's rotation undone, the
// V switch undone, ACC. T0 = the RF, T1 = the line table. u0 = (-, W); f0 = (fcu_n, cu_line).

fn cu_phase(j: i32, yf: f32, rot: f32) -> f32 {
    let l = P.f0.y * yf + rot * yf * 0.25;
    return P.f0.x * f32(j) + (l - floor(l));
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let n = i32(floor(pos.x));
    let y = i32(floor(pos.y));
    let yf = ld1f(12, y);
    let rot = ld1f(10, y);
    var i0 = 0.0;
    var q0 = 0.0;
    var th = 0.0;
    var x = 0.0;
    // @TAPS CHROMA th = cu_phase(n + K_CHROMA_C - {i}, yf, rot); x = ld0f(n + K_CHROMA_C - {i}, y); i0 = i0 + {c} * (x * cos_cyc(th)); q0 = q0 + {c} * (x * sin_cyc(th));
    let acc = ld1f(9, y);
    let uu = 2.0 * i0 * acc;
    let vv = 2.0 * q0 * acc * ld1f(11, y);
    return put2(uu, 2.0, 16384.0, vv, 2.0, 16384.0);
}
