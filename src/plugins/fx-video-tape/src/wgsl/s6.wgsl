// vhs_tape stage 6 (`sim::s6`): FM demodulation — the Hilbert FIR, the discriminator's phase step
// → level; the RF envelope. T0 = the RF. u0 = (std, W); f0 = (fsync_n, dev_n, l_sync).

fn khb(i: i32, s: u32) -> f32 {
    if (s == 0u) { return K_HILB_P[i]; }
    return K_HILB_N[i];
}

fn analytic(j: i32, y: i32, s: u32) -> vec2<f32> {
    var q = 0.0;
    for (var i = 0; i < K_HILB_P_LEN; i = i + 1) {
        let d = 2 * i + 1;
        q = q + khb(i, s) * (ld0f(j - d, y) - ld0f(j + d, y));
    }
    return vec2<f32>(ld0f(j, y), q);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let n = i32(floor(pos.x));
    let y = i32(floor(pos.y));
    let s = P.u0.x;
    let z0 = analytic(n, y, s);
    let z1 = analytic(n - 1, y, s);
    let re = z0.x * z1.x + z0.y * z1.y;
    let im = z0.y * z1.x - z0.x * z1.y;
    let d = atan2_cyc(im, re);
    let level = P.f0.z + (d - P.f0.x) / P.f0.y;
    let env = sqrt(z0.x * z0.x + z0.y * z0.y);
    return put2(level, 2.0, 8192.0, env, 0.0, 16384.0);
}
