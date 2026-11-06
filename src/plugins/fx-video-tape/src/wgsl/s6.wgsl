// vhs_tape stage 6 (`sim::s6`): FM demodulation — the Hilbert FIR, the discriminator's phase step
// → level; the RF envelope. T0 = the RF. u0 = (-, W); f0 = (fsync_n, dev_n, l_sync).

fn analytic(j: i32, y: i32) -> vec2<f32> {
    var q = 0.0;
    // @TAPS HILB q = q + {c} * (ld0f(j - 2 * {i} - 1, y) - ld0f(j + 2 * {i} + 1, y));
    return vec2<f32>(ld0f(j, y), q);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let n = i32(floor(pos.x));
    let y = i32(floor(pos.y));
    let z0 = analytic(n, y);
    let z1 = analytic(n - 1, y);
    let re = z0.x * z1.x + z0.y * z1.y;
    let im = z0.y * z1.x - z0.x * z1.y;
    let d = atan2_cyc(im, re);
    let level = P.f0.z + (d - P.f0.x) / P.f0.y;
    let env = sqrt(z0.x * z0.x + z0.y * z0.y);
    return put2(level, 2.0, 8192.0, env, 0.0, 16384.0);
}
